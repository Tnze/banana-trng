use embassy_stm32::{flash::Flash, peripherals::FLASH, Peri};
use sequential_storage::{
    cache::KeyCacheImpl,
    map::{self, Key, Value},
};

pub use sequential_storage::cache::*;
const RANGE: core::ops::Range<u32> = 0x0001F000..0x00020000;

pub type Error = sequential_storage::Error<embassy_stm32::flash::Error>;

mod wrapper {
    use embassy_stm32::flash::{
        Blocking, Error, Flash, FLASH_SIZE, MAX_ERASE_SIZE, READ_SIZE, WRITE_SIZE,
    };

    pub(super) struct FakeAsyncFlash<'a>(pub(super) Flash<'a, Blocking>);

    impl<'a> embedded_storage_async::nor_flash::ErrorType for FakeAsyncFlash<'a> {
        type Error = Error;
    }

    impl<'a> embedded_storage_async::nor_flash::ReadNorFlash for FakeAsyncFlash<'a> {
        const READ_SIZE: usize = READ_SIZE;

        async fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
            self.0.blocking_read(offset, bytes)
        }

        fn capacity(&self) -> usize {
            FLASH_SIZE
        }
    }

    impl<'a> embedded_storage_async::nor_flash::NorFlash for FakeAsyncFlash<'a> {
        const WRITE_SIZE: usize = WRITE_SIZE;
        const ERASE_SIZE: usize = MAX_ERASE_SIZE;

        async fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
            self.0.blocking_erase(from, to)
        }

        async fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
            self.0.blocking_write(offset, bytes)
        }
    }
}

pub struct Storage<C, const N: usize> {
    flash: wrapper::FakeAsyncFlash<'static>,
    cache: C,
    buffer: [u8; N],
}

impl<C, const N: usize> Storage<C, N> {
    pub fn new(flash: Peri<'static, FLASH>, cache: C) -> Self {
        Self {
            flash: wrapper::FakeAsyncFlash(Flash::new_blocking(flash)),
            cache,
            buffer: [0; N],
        }
    }

    pub async fn read<'a, K, V>(&'a mut self, key: &K) -> Result<Option<V>, Error>
    where
        K: Key,
        V: Value<'a>,
        C: KeyCacheImpl<K>,
    {
        map::fetch_item(
            &mut self.flash,
            RANGE,
            &mut self.cache,
            &mut self.buffer,
            key,
        )
        .await
    }

    pub async fn write<'a, K, V>(&'a mut self, key: &K, value: &V) -> Result<(), Error>
    where
        K: Key,
        V: Value<'a>,
        C: KeyCacheImpl<K>,
    {
        map::store_item(
            &mut self.flash,
            RANGE,
            &mut self.cache,
            &mut self.buffer,
            key,
            value,
        )
        .await
    }

    pub async fn read_or_default<'a, 'b, K, V>(
        &'a mut self,
        key: &K,
        default: V,
    ) -> Result<V, Error>
    where
        'a: 'b,
        K: Key,
        V: Value<'b>,
        C: KeyCacheImpl<K>,
    {
        Ok(self.read(key).await?.unwrap_or(default))
    }
}
