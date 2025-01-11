use std::default::Default;
use std::ops::{Index, IndexMut};
use std::sync::{Arc, RwLock};

use crossbeam::channel::{Sender, TrySendError};

/// the devices connected to the emulator's peripheral bus
///
/// the lawa isa actually supports up to 255 devices on the peripheral bus, but we maintain an
/// array of 256 devices to simplify indexing (the device index 0 is illegal, as when devices
/// trigger interrupts, the low byte of the interrupt context is set to the device index of the
/// triggering device, but software-triggered interrupts set the low byte of the interrupt context
/// to 0)
pub struct Devices([Option<Box<dyn Device>>; 256]);

impl Index<u8> for Devices {
    type Output = Option<Box<dyn Device>>;

    fn index(&self, index: u8) -> &Self::Output {
        if index == 0 {
            panic!("device index 0 is reserved, and reading input from it or writing output to it is not allowed")
        } else {
            &self.0[usize::from(index)]
        }
    }
}

impl IndexMut<u8> for Devices {
    fn index_mut(&mut self, index: u8) -> &mut Self::Output {
        if index == 0 {
            panic!("device index 0 is reserved, and reading input from it or writing output to it is not allowed")
        } else {
            &mut self.0[usize::from(index)]
        }
    }
}

impl Default for Devices {
    fn default() -> Self {
        Self([const { None }; 256])
    }
}

/// a device which may be connected to an emulator's peripheral bus
///
/// a device is responsible for implementing three methods: one for initialisation of the device,
/// one which may be used to receive input from the device, and one which may be used to send
/// output to the device. the initialisation function is provided with a handle which the device
/// may use to attempt to trigger a hardware interrupt
pub trait Device {
    fn init(&mut self, interrupt_handle: InterruptHandle);

    fn input(&mut self, context: u8) -> u16;
    fn output(&mut self, context: u8, value: u16);
}

/// a handle which a device may use to attempt to trigger an interrupt
///
/// every device attached to lawa's peripheral bus may attempt to trigger an interrupt. as such,
/// whenever a device in the emulator is initialised, it is passed a handle which it may use to
/// attempt to trigger an interrupt. note that attempting to trigger an interrupt is non-blocking
/// and also fallible. attempting to trigger an interrupt may fail if another device has already
/// triggered an interrupt which the cpu has not yet cleared, or if interrupts from the device to
/// which this handle belongs are masked
pub struct InterruptHandle {
    device_index: u8,

    sender: Sender<u16>,
    interrupt_mask: Arc<RwLock<[u16; 16]>>,
}

impl InterruptHandle {
    pub fn new(
        device_index: u8,
        sender: Sender<u16>,
        interrupt_mask: Arc<RwLock<[u16; 16]>>,
    ) -> Self {
        Self {
            device_index,
            sender,
            interrupt_mask,
        }
    }

    /// attempt to trigger a hardware interrupt
    ///
    /// a device may call this method on its provided interrupt handle at any time to attempt to
    /// trigger an hardware interrupt. attempting an interrupt is non-blocking and fallible, so a
    /// device wishing to ensure an interrupt is triggered may need to repeatedly attempt to
    /// triggger interrupts until it succeeds
    pub fn try_interrupt(&self, interrupt_context: u8) -> Result<(), TryInterruptError> {
        // FIXME: this probably shouldn't unwrap
        let interrupt_mask = self.interrupt_mask.try_read().unwrap();

        // check if either the global interrupt mask bit or this device's specific interrupt bit is
        // set
        let all_interrupts_masked = (interrupt_mask[0] & 0x0001) != 0;
        let interrupt_masked = interrupt_mask[usize::from(self.device_index >> 4)]
            & (1 << (self.device_index & 0x0F))
            != 0;

        // if interrupts for this device are masked, do not attempt to send the interrupt to the
        // cpu
        if all_interrupts_masked || interrupt_masked {
            return Err(TryInterruptError::InterruptMasked);
        }

        Ok(self
            .sender
            .try_send(u16::from_le_bytes([self.device_index, interrupt_context]))?)
    }
}

/// an error resulting from a failed attempt to trigger an interrupt
///
/// a device attempting to trigger a hardware interrupt is a fallible operation. in particular, it
/// may fail either because the cpu has already received an interrupt which it has not yet cleared,
/// or else because the device attempting to trigger the interrupt is masked, either by the global
/// interrupt mask or by its specific interrupt mask bit
pub enum TryInterruptError {
    TrySendError(TrySendError<u16>),
    InterruptMasked,
}

impl From<TrySendError<u16>> for TryInterruptError {
    fn from(value: TrySendError<u16>) -> Self {
        Self::TrySendError(value)
    }
}
