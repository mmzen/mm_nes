use std::fs::File;
use std::io::{copy, Write};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use log::{debug, info};
use mmretrodb::nes_rom_metadata::NesRomMetadata;
use mmretrodb::rdb::Rdb;

const RDB_FILE_NAME: &str = "nes_roms.rdb";
const RDB_URL: &str = "https://github.com/libretro/libretro-database/raw/refs/heads/master/rdb/Nintendo%20-%20Nintendo%20Entertainment%20System.rdb";

#[derive(Debug)]
pub enum NesRomMetadataWorkerError {
    InitializationError(String),
    CommunicationError(String),
    InternalError(String),
    SearchError(String),
}

#[derive(Debug)]
pub enum NesRomMetadataMessage {
    RequestMetadataByCrc(u32),
    ResponseMetadata(Option<NesRomMetadata>),
    Error(String),
    Dummy
}

pub struct NesRomMetadataWorker {
    request_tx: Sender<NesRomMetadataMessage>,
    response_rx: Receiver<NesRomMetadataMessage>,
    pub handle: Option<JoinHandle<()>>,
}

impl NesRomMetadataWorker {

    fn build_http_client() -> Result<reqwest::blocking::Client, NesRomMetadataWorkerError> {
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .or(Err(NesRomMetadataWorkerError::InitializationError("could not build HTTP client".to_string())))
    }

    fn send_http_request(client: reqwest::blocking::Client, file: &str) -> Result<reqwest::blocking::Response, NesRomMetadataWorkerError> {
        debug!("sending HTTP request to: {} ...", file);

        let response = client.get(file)
            .send()
            .map_err(|err| NesRomMetadataWorkerError::CommunicationError(format!("could not send request: {}", err)))?
            .error_for_status()
            .map_err(|err| NesRomMetadataWorkerError::CommunicationError(format!("could not receive response: {}", err)))?;

        Ok(response)
    }

    fn download(file: &str) -> Result<PathBuf, NesRomMetadataWorkerError> {
        let client = NesRomMetadataWorker::build_http_client()?;
        let mut response = NesRomMetadataWorker::send_http_request(client, file)?;

        let path = std::env::temp_dir().join(RDB_FILE_NAME);

        let mut file = File::create(&path)
            .map_err(|e| NesRomMetadataWorkerError::CommunicationError(format!("could not create file {}: {}", path.display(), e)))?;

        debug!("downloading NES rdb file to: {} ...", path.display());

        copy(&mut response, &mut file)
            .map_err(|e| NesRomMetadataWorkerError::CommunicationError(format!("could not write file {}: {}", path.display(), e)))?;

        file.flush()
            .map_err(|e| NesRomMetadataWorkerError::CommunicationError(format!("could not save file {}: {}", path.display(), e)))?;

        Ok(path)
    }

    fn init_rdb() -> Result<Rdb, NesRomMetadataWorkerError> {
        debug!("loading NES rdb file ...");

        let rdb_file = NesRomMetadataWorker::download(RDB_URL)?;
        let rdb = Rdb::open(rdb_file).map_err(|e| NesRomMetadataWorkerError::InitializationError(e.to_string()))?;

        Ok(rdb)
    }

    fn loop_not_ready(request_rx: Receiver<NesRomMetadataMessage>, response_tx: Sender<NesRomMetadataMessage>) {

        loop {
            let response = match request_rx.recv() {
                _ => NesRomMetadataMessage::Error("not ready".to_string()),
            };

            let _ = response_tx.send(response);
        }
    }

    fn find_metadata_by_crc(crc: u32, rdb: &mut Rdb) -> Result<Option<NesRomMetadata>, NesRomMetadataWorkerError> {
        rdb.scan_by_crc(crc).map_err(|e| NesRomMetadataWorkerError::SearchError(e.to_string()))
    }

    fn loop_ready(request_rx: Receiver<NesRomMetadataMessage>, response_tx: Sender<NesRomMetadataMessage>, mut rdb: Rdb) {
        loop {
            let response = match request_rx.recv() {
                Ok(NesRomMetadataMessage::RequestMetadataByCrc(crc)) => {
                    let result = NesRomMetadataWorker::find_metadata_by_crc(crc, &mut rdb);

                    match result {
                        Ok(Some(metadata)) => NesRomMetadataMessage::ResponseMetadata(Some(metadata)),
                        Ok(None) => NesRomMetadataMessage::ResponseMetadata(None),
                        Err(_) => NesRomMetadataMessage::Error("error searching for metadata".to_string()),
                    }
                },

                Err(e) => NesRomMetadataMessage::Error(e.to_string()),
                _ => NesRomMetadataMessage::Error("unsupported message received".to_string()),
            };

            let _ = response_tx.send(response);
        }
    }

    pub fn spawn() -> Result<NesRomMetadataWorker, NesRomMetadataWorkerError> {
        let (request_tx, request_rx) = channel::<NesRomMetadataMessage>();
        let (response_tx, response_rx) = channel::<NesRomMetadataMessage>();

        let handle = thread::Builder::new()
            .name("nes_rom_metadata_worker".to_string())
            .spawn(move || {

                let result = NesRomMetadataWorker::init_rdb();
                debug!("=> {:?}", result);

                match result {
                    Ok(rdb) => NesRomMetadataWorker::loop_ready(request_rx, response_tx, rdb),
                    Err(_) => NesRomMetadataWorker::loop_not_ready(request_rx, response_tx),
                }
            })
            .map_err(|e| NesRomMetadataWorkerError::InternalError(e.to_string()))?;

        info!("NES ROM metadata worker started...");
        Ok(NesRomMetadataWorker { request_tx, response_rx, handle: Some(handle) })
    }

    pub fn request(&self, crc: u32) -> Result<(), NesRomMetadataWorkerError> {
        self.request_tx.send(NesRomMetadataMessage::RequestMetadataByCrc(crc))
            .map_err(|e| NesRomMetadataWorkerError::CommunicationError(e.to_string()))
    }

    pub fn try_recv(&self) -> Option<NesRomMetadataMessage> {
        self.response_rx.try_recv().ok()
    }
}