// Authorship: Human 100% | Claude 0%
use std::thread::sleep;
use std::time::Duration;
use crate::nes_rom_metadata_worker::{NesRomMetadataMessage, NesRomMetadataWorker};
use crate::tests::init;

#[ignore]
#[test]
fn test_metadata() {
    init();
    
    let result = NesRomMetadataWorker::spawn();

    if let Ok(metadata) = result {
        let _ = metadata.request(872268823);
        sleep(Duration::from_secs(10));
        let response = metadata.try_recv().unwrap();

        if let NesRomMetadataMessage::ResponseMetadata(Some(rom_metadata)) = response {
            println!("{}", rom_metadata);
        }
        let _ = metadata.handle.unwrap().join();
    }
}