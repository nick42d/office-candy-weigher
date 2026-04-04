use crate::{
    config_consts::{DEFAULT_LOLLY_WEIGHT, DEFAULT_SCALE_RAW_50G, DEFAULT_SCALE_RAW_TARE},
    state::round_f32,
    Irqs, FLASH_STORAGE_OFFSET_BYTES,
};
use defmt::{error, info};
use embassy_rp::{
    dma,
    flash::{Async, Flash, ERASE_SIZE, PAGE_SIZE, READ_SIZE, WRITE_SIZE},
    peripherals::{self, DMA_CH0, DMA_CH1, FLASH},
    Peri,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

const PICO_FLASH_SIZE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Deserialize, Serialize, Debug, PartialEq, defmt::Format)]
pub struct Config {
    pub tare_weight_dg: i32,
    pub lolly_weight_dg: i32,
    pub saved_tared_scale_weight: i32,
    pub scale_raw_50g: f32,
    pub scale_raw_tare: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tare_weight_dg: Default::default(),
            lolly_weight_dg: round_f32(DEFAULT_LOLLY_WEIGHT * 10.0),
            saved_tared_scale_weight: Default::default(),
            scale_raw_50g: DEFAULT_SCALE_RAW_50G,
            scale_raw_tare: DEFAULT_SCALE_RAW_TARE,
        }
    }
}

pub struct FlashController<'a> {
    flash: Flash<'a, FLASH, Async, PICO_FLASH_SIZE_BYTES>,
    offset: u32,
}
impl<'a> FlashController<'a> {
    pub fn new(flash: Peri<'a, FLASH>, dma: Peri<'a, DMA_CH1>, offset: u32) -> Self {
        let flash = Flash::new(flash, dma, Irqs);
        Self { flash, offset }
    }
    /// Warning - this could return random noise, provided it deserialised into
    /// a T.
    pub async fn read<T: DeserializeOwned, const SIZE_BYTES: usize>(
        &mut self,
    ) -> postcard::Result<T> {
        const {
            assert!(
                SIZE_BYTES.is_multiple_of(READ_SIZE),
                "SIZE_BYTES should be a multiple of read size"
            );
        }
        let mut buf = [0u8; SIZE_BYTES];
        if let Err(e) = self.flash.read(self.offset, &mut buf).await {
            error!("Error reading flash: {}", e);
        }
        postcard::from_bytes::<T>(&buf)
    }
    pub fn write<T: Serialize + defmt::Format, const MAX_WRITE_SIZE_BYTES: usize>(
        &mut self,
        t: &T,
    ) {
        const {
            assert!(
                MAX_WRITE_SIZE_BYTES.is_multiple_of(PAGE_SIZE),
                "MAX_WRITE_SIZE_BYTES should be a multiple of page size"
            );
            assert!(
                MAX_WRITE_SIZE_BYTES.is_multiple_of(ERASE_SIZE),
                "MAX_WRITE_SIZE_BYTES should be a multiple of sector size"
            );
        }
        info!("Attempting to write {} to flash", t);
        info!("Erasing flash STORAGE section");
        self.flash
            .blocking_erase(
                self.offset,
                self.offset + u32::try_from(MAX_WRITE_SIZE_BYTES).unwrap(),
            )
            .unwrap();
        info!("Flash STORAGE section erased");

        let mut buf = [0xFF; MAX_WRITE_SIZE_BYTES];
        let used_slice = postcard::to_slice(&t, &mut buf).unwrap();
        info!(
            "Postcard compressed the struct to {} bytes",
            used_slice.len()
        );

        info!("Writing to flash STORAGE section");
        self.flash.blocking_write(self.offset, &buf).unwrap();
        info!("Wrote to flash STORAGE section");
    }
}
