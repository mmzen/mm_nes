use std::thread::sleep;
use std::time::Duration;
use crate::nes_rom_metadata_worker::NesRomMetadataWorker;
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

        println!("metadata: {:?}", response);
        let _ = metadata.handle.unwrap().join();
    }
}