use crate::RawDataU64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Regs<'a> {
    regs_mask: u64,
    raw_regs: RawDataU64<'a>,
}

impl<'a> Regs<'a> {
    pub fn new(regs_mask: u64, raw_regs: RawDataU64<'a>) -> Self {
        Self {
            regs_mask,
            raw_regs,
        }
    }

    pub fn get(&self, register: u64) -> Option<u64> {
        if self.regs_mask & (1 << register) == 0 {
            return None;
        }

        let mut index = 0;
        for i in 0..register {
            if self.regs_mask & (1 << i) != 0 {
                index += 1;
            }
        }
        self.raw_regs.get(index)
    }
}
