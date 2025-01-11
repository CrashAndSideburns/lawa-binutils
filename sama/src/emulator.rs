use std::default::Default;
use std::ops::{Index, IndexMut};

use crossbeam::channel::{self, Receiver, Sender};

use crate::devices::Devices;

// TODO: once an actual spec has been written for lawa, include it in the doc comment here
/// an emulator for a computer based on the lawa isa
pub struct Emulator {
    pub program_counter: u16,
    pub privileged: bool,

    pub registers: Registers,
    pub control_status_registers: ControlStatusRegisters,
    pub devices: Devices,
    pub ram: Ram,

    interrupt_receiver: Receiver<u16>,

    // we need a copy of the sender for the interrupts channel on hand so that when a new device is
    // attached we can construct an interrupt handle to send to send to it
    interrupt_sender: Sender<u16>,
}

impl Default for Emulator {
    fn default() -> Self {
        // interrupts aren't buffered, so the channel should only have a capacity of 1. in other
        // words, if the cpu receives an interrupt, no other device should be able to send one
        // until the cpu goes through a clock cycle and acknowledges the existing interrupt request
        let (interrupt_sender, interrupt_receiver) = channel::bounded(1);

        Self {
            program_counter: 0,
            privileged: true,

            registers: Registers::default(),
            control_status_registers: ControlStatusRegisters::default(),
            devices: Devices::default(),
            ram: Ram::default(),

            interrupt_receiver,
            interrupt_sender,
        }
    }
}

/// the general-purpose registers contained within a lawa cpu
///
/// lawa does not actually have 32 general-purpose registers, as the 0th register is the special
/// `zero register', to which writes are ignored and from which reads always return zero. similarly
/// to what is done for devices, we maintain an array of 32 registers to simplify indexing
#[derive(Debug, Default, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Registers(pub [u16; 32]);

impl Index<u16> for Registers {
    type Output = u16;

    fn index(&self, index: u16) -> &Self::Output {
        if index == 0 {
            &0
        } else {
            &self.0[usize::from(index)]
        }
    }
}

impl IndexMut<u16> for Registers {
    fn index_mut(&mut self, index: u16) -> &mut Self::Output {
        &mut self.0[usize::from(index)]
    }
}

/// the control/status registers contained within a lawa cpu
///
/// lawa formally has 32 control/status registers, but only 29 of them currently have specified
/// uses, with the remaining 3 reserved for potential future specified uses. it is undefined
/// behaviour to read from or write to any of the control/status registers which do not currently
/// have a specified use.
#[derive(Debug, Default, Clone, Copy, Hash, PartialEq, Eq)]
pub struct ControlStatusRegisters {
    pub im: [u16; 16],
    pub iv: u16,
    pub ipc: u16,
    pub ic: u16,
    pub mpc: [u16; 2],
    pub mpa: [u16; 8],
}

impl Index<u16> for ControlStatusRegisters {
    type Output = u16;

    fn index(&self, index: u16) -> &Self::Output {
        match index {
            0b00000..=0b01111 => &self.im[usize::from(index)],
            0b10000 => &self.iv,
            0b10001 => &self.ipc,
            0b10010 => &self.ic,
            0b10011..=0b10101 => panic!("control/status register {index} is currently undefined, and is reserved for potential future usage"),
            0b10110..=0b10111 => &self.mpc[usize::from(index & 0b00001)],
            0b11000..=0b11111 => &self.mpa[usize::from(index & 0b00111)],
            _ => unreachable!(),
        }
    }
}

impl IndexMut<u16> for ControlStatusRegisters {
    fn index_mut(&mut self, index: u16) -> &mut Self::Output {
        match index {
            0b00000..=0b01111 => &mut self.im[usize::from(index)],
            0b10000 => &mut self.iv,
            0b10001 => &mut self.ipc,
            0b10010 => &mut self.ic,
            0b10011..=0b10101 => panic!("control/status register {index} is currently undefined, and is reserved for potential future usage"),
            0b10110..=0b10111 => &mut self.mpc[usize::from(index & 0b00001)],
            0b11000..=0b11111 => &mut self.mpa[usize::from(index & 0b00111)],
            _ => unreachable!(),
        }
    }
}

/// the ram used by the emulator
///
/// the lawa isa has somewhat unusual ram, in that it has a 16-bit word size, rather than the more
/// typical 8-bit word size, and has a 16-bit address size (this is less unusual, but it means that
/// using the standard `usize'-indexed arrays requires a lot of awkward conversions). as such, a
/// ram type is implemented to encapsulate these details
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Ram(pub [u16; 0x10000]);

impl Index<u16> for Ram {
    type Output = u16;

    fn index(&self, index: u16) -> &Self::Output {
        &self.0[usize::from(index)]
    }
}

impl IndexMut<u16> for Ram {
    fn index_mut(&mut self, index: u16) -> &mut Self::Output {
        &mut self.0[usize::from(index)]
    }
}

impl Default for Ram {
    fn default() -> Self {
        Self([0; 0x10000])
    }
}

impl Emulator {
    /// return `true' iff the emulator, in its current state, has read permissions at `address'
    fn readable(&self, index: u16) -> bool {
        // TODO
        true
    }

    /// return `true' iff the emulator, in its current state, has write permissions at `address'
    fn writable(&self, index: u16) -> bool {
        // TODO
        true
    }

    /// return `true' iff the emulator, in its current state, has execute permissions at `address'
    fn executable(&self, index: u16) -> bool {
        // TODO
        true
    }

    /// set up the state of the emulator to reflect a software interrupt having occurred
    ///
    /// when stepping the emulator, an interrupt can be triggered by software in two ways: by the
    /// emulator, in user mode, attempting to perform some kind of action for which it does not
    /// have the appropriate permissions; or by the emulator, running in uses mode, requesting an
    /// interrupt via the `swpr' instruction
    ///
    /// this function simply sets the control/status registers, the program counter, and the
    /// privilege bit to the states caused by the triggering of a software-triggered interrupt, to
    /// avoid duplicated code inside of the `step' function.
    fn software_interrupt(&mut self, context: u8, instruction_length: u16) {
        // set the global interrupt mask bit
        self.control_status_registers.im[0] |= 0x01;

        // set the interrupted program counter. because this function is called before the program
        // counter is updated, we need to be passed the length of the instruction currently being
        // executed to set the interrupted program counter to the address of the next instruction
        // to be executed
        self.control_status_registers.ipc = self.program_counter.wrapping_add(instruction_length);

        // set the interrupt context. note that because this is a software interrupt, the low byte
        // of the interrupt context is 0
        self.control_status_registers.ic = u16::from_le_bytes([0x00, context]);
        
        // set the program counter to the interrupt vector
        self.program_counter = self.control_status_registers.iv;

        // set the privilege bit
        self.privileged = true;
    }

    /// executes the instruction located at the address currently in the program counter
    ///
    /// # panics
    ///
    /// calling this function will panic if executing the instruction which the program counter is
    /// pointing to depends on any behaviour which is not defined by lawa's specification. in the
    /// future, this will probably change to something more lenient, with this function instead
    /// returning some kind of error, and marking the emulator as `poisoned'.
    pub fn step(&mut self) {
        // check if any attached devices have triggered interrupts that we need to handle
        if self.interrupt_receiver.is_full() {
            // when an interrupt is triggered, we need to set the global interrupt mask *before* we
            // actually remove the interrupt from the receiver, so that a device can't successfully
            // send another interrupt in the time between the receiver being cleared and the global
            // interrupt mask being set
            self.control_status_registers.im[0] |= 0x01;

            // we can unwrap safely here, since we have already checked that the receiver is full
            let interrupt_context = self.interrupt_receiver.recv().unwrap();

            self.control_status_registers.ipc = self.program_counter;
            self.control_status_registers.ic = interrupt_context;
            self.program_counter = self.control_status_registers.iv;
            self.privileged = true;

            return;
        }

        // first, just pull apart the instruction into its parts, and grab all of the values that
        // will be useful to the various instructions
        let instr = self.ram[self.program_counter];
        let opc = instr & 0b0000000000111111;
        let src_idx = (instr & 0b1111100000000000) >> 11;
        let src = self.registers[src_idx];
        let dst_idx = (instr & 0b0000011111000000) >> 6;
        let dst = self.registers[dst_idx];

        // check if the instruction to be executed takes an immediate
        let takes_imm = ((opc & 0b001000) != 0) && (opc != 0b101001);

        // make sure that the required addresses are executable, and fetch the immediate if
        // required
        if !self.executable(self.program_counter) {
            self.software_interrupt(0b00000001, if takes_imm { 2 } else { 1 });
            return;
        }

        let imm = if takes_imm {
            if !self.executable(self.program_counter.wrapping_add(1)) {
                self.software_interrupt(0b00000001, 2);
                return;
            }

            self.ram[self.program_counter.wrapping_add(1)]
        } else {
            // NOTE: in this case, the instruction doesn't actually take an immediate at all, so
            // just return a dummy value
            0
        };

        match opc {
            0b000000 => {
                // add
                self.registers[dst_idx] = dst.wrapping_add(src);
            }
            0b000001 => {
                // sub
                self.registers[dst_idx] = dst.wrapping_sub(src);
            }
            0b000010 => {
                // and
                self.registers[dst_idx] &= src;
            }
            0b000011 => {
                // or
                self.registers[dst_idx] |= src;
            }
            0b000100 => {
                // xor
                self.registers[dst_idx] ^= src;
            }
            0b000101 => {
                // sll
                if (src as i16).is_positive() {
                    self.registers[dst_idx] <<= src
                } else {
                    self.registers[dst_idx] >>= src.wrapping_neg()
                }
            }
            0b000110 => {
                // srl
                if (src as i16).is_positive() {
                    self.registers[dst_idx] >>= src
                } else {
                    self.registers[dst_idx] <<= src.wrapping_neg()
                }
            }
            0b000111 => {
                // sra
                if (src as i16).is_positive() {
                    self.registers[dst_idx] = ((dst as i16) >> src) as u16;
                } else {
                    self.registers[dst_idx] <<= src.wrapping_neg()
                }
            }
            0b001000 => {
                // addi
                self.registers[dst_idx] = src.wrapping_add(imm);
            }

            0b001010 => {
                // andi
                self.registers[dst_idx] = src & imm;
            }
            0b001011 => {
                // ori
                self.registers[dst_idx] = src | imm;
            }
            0b001100 => {
                // xori
                self.registers[dst_idx] = src ^ imm;
            }
            0b001101 => {
                // slli
                self.registers[dst_idx] = if (imm as i16).is_positive() {
                    src << imm
                } else {
                    src >> imm.wrapping_neg()
                }
            }

            0b001111 => {
                // srai
                self.registers[dst_idx] = if (imm as i16).is_positive() {
                    ((src as i16) >> imm) as u16
                } else {
                    src << imm
                }
            }
            0b010000 => {
                // ld
                if !self.readable(src) {
                    self.software_interrupt(0b00000100, 1);
                    return;
                }

                self.registers[dst_idx] = self.ram[src];
            }
            0b010001 => {
                // st
                if !self.writable(src) {
                    self.software_interrupt(0b00000010, 1);
                    return;
                }

                self.ram[src] = dst;
            }
            0b010010 => {
                // dei
                if !self.privileged {
                    self.software_interrupt(0b00001100, 1);
                    return;
                }

                let device_index = src.to_be_bytes()[0];
                let device_context = src.to_be_bytes()[1];

                // NOTE: attempting to read input from a device index at which there is no device
                // attached to the device bus is undefined behaviour. in a hardware implementation,
                // this is likely to simply return garbage
                let device = self.devices[device_index].as_mut().expect("attempted to read input from device at index {device_index}, but no such device exists");
                self.registers[dst_idx] = device.input(device_context);
            }
            0b010011 => {
                // deo
                if !self.privileged {
                    self.software_interrupt(0b00001010, 1);
                    return;
                }

                let device_index = src.to_be_bytes()[0];
                let device_context = src.to_be_bytes()[1];
                let device = &mut self.devices[device_index];
                if let Some(ref mut device) = device {
                    device.output(device_context, dst);
                }
            }
            0b010100 => {
                // rcsr
                if !self.privileged {
                    self.software_interrupt(0b00010100, 1);
                    return;
                }

                self.registers[dst_idx] = self.control_status_registers[src_idx];
            }
            0b010101 => {
                // wcsr
                if !self.privileged {
                    self.software_interrupt(0b00010010, 1);
                    return;
                }

                self.control_status_registers[dst_idx] = src;
            }
            0b010110 => {
                // swpr
                if self.privileged {
                    self.program_counter = self.control_status_registers.ipc;
                    self.privileged = false;
                    return;
                } else {
                    self.software_interrupt(0b00000000, 1);
                    return;
                }
            }

            0b011000 => {
                // ldio
                if !self.readable(src.wrapping_add(imm)) {
                    self.software_interrupt(0b00000100, 2);
                    return;
                }

                self.registers[dst_idx] = self.ram[src.wrapping_add(imm)];
            }
            0b011001 => {
                // stio
                if !self.writable(src.wrapping_add(imm)) {
                    self.software_interrupt(0b00000010, 2);
                    return;
                }

                self.ram[src.wrapping_add(imm)] = self.registers[dst_idx];
            }

            0b101000 => {
                // jal
                self.registers[dst_idx] = self.program_counter.wrapping_add(2);
                self.program_counter = src.wrapping_add(imm);
                return;
            }
            0b101001 => {
                // jlo
                let imm = (((instr & 0b1111111111000000) as i16) >> 6) as u16;
                self.program_counter = self.program_counter.wrapping_add(imm);
                return;
            }
            0b101010 => {
                // beq
                if dst == src {
                    self.program_counter = imm;
                    return;
                }
            }
            0b101011 => {
                // bne
                if dst != src {
                    self.program_counter = imm;
                    return;
                }
            }
            0b101100 => {
                // blt
                if (dst as i16) < (src as i16) {
                    self.program_counter = imm;
                    return;
                }
            }
            0b101101 => {
                // bge
                if (dst as i16) >= (src as i16) {
                    self.program_counter = imm;
                    return;
                }
            }
            0b101110 => {
                // bltu
                if dst < src {
                    self.program_counter = imm;
                    return;
                }
            }
            0b101111 => {
                // bgeu
                if dst >= src {
                    self.program_counter = imm;
                    return;
                }
            }
            _ => {
                // NOTE: in this implementation, attempting to execute an instruction with an
                // undefined opcode causes a panic. this is undefined behaviour, so a
                // specification-compliant implementation could do anything here
                panic!("opcode {opc} is currently undefined, and is reserved for potential future usage")
            }
        }

        // NOTE: jump and branch instructions return early, as do software-triggered interrupts, so
        // we don't need to worry about this affecting the particular values to which they set the
        // program counter
        self.program_counter = self
            .program_counter
            .wrapping_add(if takes_imm { 2 } else { 1 });
    }
}
