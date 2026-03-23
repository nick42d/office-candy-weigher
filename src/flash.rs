use crate::{config_consts::DEFAULT_LOLLY_WEIGHT, FLASH_STORAGE_OFFSET_BYTES};
use defmt::{error, info};
use embassy_rp::{
    dma,
    flash::{Async, Flash},
    peripherals::{self, FLASH},
    Peri,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

const PICO_FLASH_SIZE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Deserialize, Serialize, Debug, PartialEq, defmt::Format)]
pub struct Config {
    pub tare_weight_g: f32,
    pub lolly_weight_g: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tare_weight_g: Default::default(),
            lolly_weight_g: DEFAULT_LOLLY_WEIGHT,
        }
    }
}

pub struct FlashController<'a, const OFFSET_BYTES: u32> {
    flash: Flash<'a, FLASH, Async, PICO_FLASH_SIZE_BYTES>,
}
impl<'a, const OFFSET_BYTES: u32> FlashController<'a, OFFSET_BYTES> {
    pub fn new(flash: Peri<'a, FLASH>, dma: Peri<'a, impl dma::Channel>) -> Self {
        let flash = Flash::new(flash, dma);
        Self { flash }
    }
    pub async fn read<T: DeserializeOwned, const SIZE_BYTES: usize>(
        &mut self,
    ) -> postcard::Result<T> {
        let mut buf = [0u8; SIZE_BYTES];
        if let Err(e) = self.flash.read(OFFSET_BYTES, &mut buf).await {
            error!("Error reading flash: {}", e);
        }
        postcard::from_bytes::<T>(&buf)
    }
    pub fn write<T: Serialize + defmt::Format, const SIZE_BYTES: usize>(&mut self, t: &T) {
        const PAGE_BUFFER_SIZE_BYTES: usize = 256;
        const SECTOR_SIZE_BYTES: usize = 4096;
        const {
            assert!(
                SIZE_BYTES.is_multiple_of(PAGE_BUFFER_SIZE_BYTES),
                "SIZE_BYTES should be a multiple of page size"
            );
            assert!(
                SIZE_BYTES.is_multiple_of(SECTOR_SIZE_BYTES),
                "SIZE_BYTES should be a multiple of sector size"
            );
        }
        info!("Attempting to write {} to flash", t);
        info!("Erasing flash STORAGE section");
        self.flash
            .blocking_erase(
                OFFSET_BYTES,
                OFFSET_BYTES + u32::try_from(SIZE_BYTES).unwrap(),
            )
            .unwrap();
        info!("Flash STORAGE section erased");

        let mut buf = [0xFF; SIZE_BYTES];
        let used_slice = postcard::to_slice(&t, &mut buf).unwrap();
        info!(
            "Postcard compressed the struct to {} bytes",
            used_slice.len()
        );

        info!("Writing to flash STORAGE section");
        self.flash.blocking_write(OFFSET_BYTES, &buf).unwrap();
        info!("Wrote to flash STORAGE section");
    }
}
