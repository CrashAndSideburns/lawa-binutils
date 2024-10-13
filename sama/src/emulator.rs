use std::default::Default;
use std::ops::{Index, IndexMut};

pub struct Emulator {
    pub program_counter: u16,
    pub privileged: bool,

    pub registers: Registers,
    pub control_status_registers: ControlStatusRegisters,
    pub devices: Devices,
    pub ram: Ram,
}

impl Default for Emulator {
    fn default() -> Self {
        Self {
            program_counter: 0,
            privileged: true,

            registers: Registers::default(),
            control_status_registers: ControlStatusRegisters::default(),
            devices: Devices::default(),
            ram: Ram::default(),
        }
    }
}

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

pub trait Device {
    fn input(&mut self, context: u8) -> u16;
    fn output(&mut self, context: u8, value: u16);
}

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
    fn readable(&self, index: u16) -> bool {
        // TODO
        true
    }

    fn writable(&self, index: u16) -> bool {
        // TODO
        true
    }

    fn executable(&self, index: u16) -> bool {
        // TODO
        true
    }

    fn interrupt(&mut self, context: u8, instruction_length: u16) {
        self.control_status_registers.ipc = self.program_counter.wrapping_add(instruction_length);
        self.control_status_registers.ic = u16::from(context) << 8;
        self.program_counter = self.control_status_registers.iv;
        self.privileged = true;
    }

    pub fn step(&mut self) {
        // First, just pull apart the instruction into its parts, and grab all of the values that
        // will be useful to the various instructions.
        let instr = self.ram[self.program_counter];
        let opc = instr & 0b0000000000111111;
        let src_idx = (instr & 0b1111100000000000) >> 11;
        let src = self.registers[src_idx];
        let dst_idx = (instr & 0b0000011111000000) >> 6;
        let dst = self.registers[dst_idx];

        // Check if the instruction to be executed takes an immediate.
        let takes_imm = ((opc & 0b001000) != 0) && (opc != 0b101001);

        // Make sure that the required addresses are executable, and fetch the immediate if
        // required.
        if !self.executable(self.program_counter) {
            self.interrupt(0b00000001, if takes_imm { 2 } else { 1 });
            return;
        }

        let imm = if takes_imm {
            if !self.executable(self.program_counter.wrapping_add(1)) {
                self.interrupt(0b00000001, 2);
                return;
            }

            self.ram[self.program_counter.wrapping_add(1)]
        } else {
            // NOTE: In this case, the instruction doesn't actually take an immediate at all, so
            // just return a dummy value.
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
                    self.interrupt(0b00000100, 1);
                    return;
                }

                self.registers[dst_idx] = self.ram[src];
            }
            0b010001 => {
                // st
                if !self.writable(src) {
                    self.interrupt(0b00000010, 1);
                    return;
                }

                self.ram[src] = dst;
            }
            0b010010 => {
                // dei
                if !self.privileged {
                    self.interrupt(0b00001100, 1);
                    return;
                }

                let device_index = src.to_be_bytes()[0];
                let device_context = src.to_be_bytes()[1];

                // NOTE: Attempting to read input from a device index at which there is no device
                // attached to the device bus is UB. In a practical hardware implementation, this
                // is likely to simply return garbage.
                let device = self.devices[device_index].as_mut().expect("attempted to read intput from device at index {device_index}, but no such device exists");
                self.registers[dst_idx] = device.input(device_context);
            }
            0b010011 => {
                // deo
                if !self.privileged {
                    self.interrupt(0b00001010, 1);
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
                    self.interrupt(0b00010100, 1);
                    return;
                }

                self.registers[dst_idx] = self.control_status_registers[src_idx];
            }
            0b010101 => {
                // wcsr
                if !self.privileged {
                    self.interrupt(0b00010010, 1);
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
                    self.interrupt(0b00000000, 1);
                    return;
                }
            }

            0b011000 => {
                // ldio
                if !self.readable(src.wrapping_add(imm)) {
                    self.interrupt(0b00000100, 2);
                    return;
                }

                self.registers[dst_idx] = self.ram[src.wrapping_add(imm)];
            }
            0b011001 => {
                // stio
                if !self.writable(src.wrapping_add(imm)) {
                    self.interrupt(0b00000010, 2);
                    return;
                }

                self.ram[src.wrapping_add(imm)] = self.registers[dst_idx];
            }

            0b101000 => {
                // jal
                self.registers[dst_idx] = self.program_counter.wrapping_add(2);
                self.program_counter = self.program_counter.wrapping_add(src).wrapping_add(imm);
                return;
            }
            0b101001 => {
                // jlo
                let imm = (instr & 0b1111111111000000) >> 6;
                self.program_counter =
                    self.program_counter
                        .wrapping_add(if (imm & 0b1000000000) == 0 {
                            imm
                        } else {
                            imm.wrapping_neg()
                        });
                return;
            }
            0b101010 => {
                // beq
                if dst == src {
                    self.program_counter = self.program_counter.wrapping_add(imm);
                    return;
                }
            }
            0b101011 => {
                // bne
                if dst != src {
                    self.program_counter = self.program_counter.wrapping_add(imm);
                    return;
                }
            }
            0b101100 => {
                // blt
                if (dst as i16) < (src as i16) {
                    self.program_counter = self.program_counter.wrapping_add(imm);
                    return;
                }
            }
            0b101101 => {
                // bge
                if (dst as i16) >= (src as i16) {
                    self.program_counter = self.program_counter.wrapping_add(imm);
                    return;
                }
            }
            0b101110 => {
                // bltu
                if dst < src {
                    self.program_counter = self.program_counter.wrapping_add(imm);
                    return;
                }
            }
            0b101111 => {
                // bgeu
                if dst >= src {
                    self.program_counter = self.program_counter.wrapping_add(imm);
                    return;
                }
            }
            _ => {
                // NOTE: In this implementation, undefined opcodes cause a panic. This is UB, so a
                // compliant implementation could do anything here.
                panic!("opcode {opc} is currently undefined, and is reserved for potential future usage")
            }
        }

        // NOTE: Jump and branch instructions return early, so we don't need to worry about this
        // affecting them.
        self.program_counter = self
            .program_counter
            .wrapping_add(if takes_imm { 2 } else { 1 });
    }
}
