//! The yatare stack VM: 14 opcodes, fixed storage, zero allocation per step.
//!
//! Semantics: one untagged `[i32; 256]` operand stack, `[i32; 16]` locals,
//! wrapping 32-bit arithmetic, explicit control flow. `RET` pops the program
//! result and halts; falling off the end also halts (top of stack, `0` if
//! empty). A step budget guards against livelocks (`jmp self`).

pub const STACK_CAP: usize = 256;
pub const N_LOCALS: usize = 16;
pub const DEFAULT_BUDGET: u64 = 10_000;

/// A single instruction. Jump targets are resolved instruction indices;
/// `prog.len()` (one past the end) is a legal target and halts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Instr {
    Push(i32),
    Pop,
    Load(u8),
    Store(u8),
    Add,
    Sub,
    Mul,
    Xor,
    Dup,
    Swap,
    Jeq(usize),
    Jlt(usize),
    Jmp(usize),
    Ret,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Trap {
    StackOverflow,
    StackUnderflow,
    BadLocal(u8),
    BadPc(usize),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Halt(i32),
    Trap(Trap),
    StepLimit,
}

#[derive(Clone)]
pub struct Vm {
    stack: [i32; STACK_CAP],
    sp: usize,
    locals: [i32; N_LOCALS],
    pc: usize,
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

impl Vm {
    pub fn new() -> Self {
        Self {
            stack: [0; STACK_CAP],
            sp: 0,
            locals: [0; N_LOCALS],
            pc: 0,
        }
    }

    pub fn set_locals(&mut self, locals: &[i32; N_LOCALS]) {
        self.locals = *locals;
    }

    pub fn locals(&self) -> &[i32; N_LOCALS] {
        &self.locals
    }

    /// Run `prog` from pc 0 with an empty stack (locals persist). Resets the
    /// stack and pc only, so callers control state injection via `set_locals`.
    pub fn run(&mut self, prog: &[Instr], budget: u64) -> Outcome {
        self.sp = 0;
        self.pc = 0;
        for _ in 0..budget {
            if self.pc == prog.len() {
                let v = if self.sp == 0 {
                    0
                } else {
                    self.stack[self.sp - 1]
                };
                return Outcome::Halt(v);
            }
            if self.pc > prog.len() {
                return Outcome::Trap(Trap::BadPc(self.pc));
            }
            match self.exec_one(prog) {
                Ok(None) => {}
                Ok(Some(v)) => return Outcome::Halt(v),
                Err(t) => return Outcome::Trap(t),
            }
        }
        Outcome::StepLimit
    }

    #[inline]
    fn push(&mut self, v: i32) -> Result<(), Trap> {
        if self.sp >= STACK_CAP {
            return Err(Trap::StackOverflow);
        }
        self.stack[self.sp] = v;
        self.sp += 1;
        Ok(())
    }

    #[inline]
    fn pop(&mut self) -> Result<i32, Trap> {
        if self.sp == 0 {
            return Err(Trap::StackUnderflow);
        }
        self.sp -= 1;
        Ok(self.stack[self.sp])
    }

    #[inline]
    fn jump(&mut self, target: usize, prog_len: usize) -> Result<(), Trap> {
        if target > prog_len {
            return Err(Trap::BadPc(target));
        }
        self.pc = target;
        Ok(())
    }

    #[inline]
    fn binop(&mut self, f: impl FnOnce(i32, i32) -> i32) -> Result<(), Trap> {
        let b = self.pop()?;
        let a = self.pop()?;
        self.push(f(a, b))
    }

    /// Execute the instruction at `self.pc`. `Ok(Some(v))` = `RET` halts
    /// with `v`; `Ok(None)` = continue.
    fn exec_one(&mut self, prog: &[Instr]) -> Result<Option<i32>, Trap> {
        let ins = prog[self.pc];
        self.pc += 1;
        match ins {
            Instr::Push(v) => self.push(v).map(|()| None),
            Instr::Pop => self.pop().map(|_| None),
            Instr::Load(l) => {
                let i = l as usize;
                if i >= N_LOCALS {
                    return Err(Trap::BadLocal(l));
                }
                let v = self.locals[i];
                self.push(v).map(|()| None)
            }
            Instr::Store(l) => {
                let i = l as usize;
                if i >= N_LOCALS {
                    return Err(Trap::BadLocal(l));
                }
                let v = self.pop()?;
                self.locals[i] = v;
                Ok(None)
            }
            Instr::Add => self.binop(i32::wrapping_add).map(|()| None),
            Instr::Sub => self.binop(i32::wrapping_sub).map(|()| None),
            Instr::Mul => self.binop(i32::wrapping_mul).map(|()| None),
            Instr::Xor => self.binop(|a, b| a ^ b).map(|()| None),
            Instr::Dup => {
                if self.sp == 0 {
                    return Err(Trap::StackUnderflow);
                }
                let v = self.stack[self.sp - 1];
                self.push(v).map(|()| None)
            }
            Instr::Swap => {
                if self.sp < 2 {
                    return Err(Trap::StackUnderflow);
                }
                self.stack.swap(self.sp - 1, self.sp - 2);
                Ok(None)
            }
            Instr::Jeq(target) => {
                let b = self.pop()?;
                let a = self.pop()?;
                if a == b {
                    self.jump(target, prog.len())?;
                }
                Ok(None)
            }
            Instr::Jlt(target) => {
                let b = self.pop()?;
                let a = self.pop()?;
                if a < b {
                    self.jump(target, prog.len())?;
                }
                Ok(None)
            }
            Instr::Jmp(target) => {
                self.jump(target, prog.len())?;
                Ok(None)
            }
            Instr::Ret => self.pop().map(Some),
        }
    }
}

/// Convenience: run `prog` with the given initial locals, returning the
/// outcome together with the final locals (the oracle's observable pair).
pub fn exec(
    prog: &[Instr],
    locals_in: &[i32; N_LOCALS],
    budget: u64,
) -> (Outcome, [i32; N_LOCALS]) {
    let mut vm = Vm::new();
    vm.set_locals(locals_in);
    let out = vm.run(prog, budget);
    (out, *vm.locals())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(prog: &[Instr]) -> Outcome {
        Vm::new().run(prog, DEFAULT_BUDGET)
    }

    #[test]
    fn push_add_ret() {
        let prog = [Instr::Push(20), Instr::Push(22), Instr::Add, Instr::Ret];
        assert_eq!(run(&prog), Outcome::Halt(42));
    }

    #[test]
    fn wrapping_add_and_sub() {
        let prog = [
            Instr::Push(i32::MAX),
            Instr::Push(1),
            Instr::Add,
            Instr::Ret,
        ];
        assert_eq!(run(&prog), Outcome::Halt(i32::MIN));
        let prog = [
            Instr::Push(i32::MIN),
            Instr::Push(1),
            Instr::Sub,
            Instr::Ret,
        ];
        assert_eq!(run(&prog), Outcome::Halt(i32::MAX));
    }

    #[test]
    fn wrapping_mul() {
        // 65536 * 65536 = 2^32 == 0 (mod 2^32)
        let prog = [
            Instr::Push(65536),
            Instr::Push(65536),
            Instr::Mul,
            Instr::Ret,
        ];
        assert_eq!(run(&prog), Outcome::Halt(0));
    }

    #[test]
    fn xor_semantics() {
        let prog = [
            Instr::Push(0b1100),
            Instr::Push(0b1010),
            Instr::Xor,
            Instr::Ret,
        ];
        assert_eq!(run(&prog), Outcome::Halt(0b0110));
    }

    #[test]
    fn load_store_roundtrip_updates_observable_locals() {
        let prog = [
            Instr::Push(7),
            Instr::Store(3),
            Instr::Push(9),
            Instr::Store(0),
            Instr::Load(3),
            Instr::Load(0),
            Instr::Add,
            Instr::Ret,
        ];
        let (out, locals) = exec(&prog, &[0; N_LOCALS], DEFAULT_BUDGET);
        assert_eq!(out, Outcome::Halt(16));
        assert_eq!(locals[3], 7);
        assert_eq!(locals[0], 9);
    }

    #[test]
    fn initial_locals_are_injected() {
        let prog = [Instr::Load(5), Instr::Load(6), Instr::Mul, Instr::Ret];
        let mut init = [0; N_LOCALS];
        init[5] = 6;
        init[6] = 7;
        let (out, _) = exec(&prog, &init, DEFAULT_BUDGET);
        assert_eq!(out, Outcome::Halt(42));
    }

    #[test]
    fn dup_doubles() {
        let prog = [Instr::Push(5), Instr::Dup, Instr::Add, Instr::Ret];
        assert_eq!(run(&prog), Outcome::Halt(10));
    }

    #[test]
    fn swap_exchanges_top_two() {
        let prog = [Instr::Push(1), Instr::Push(2), Instr::Swap, Instr::Ret];
        assert_eq!(run(&prog), Outcome::Halt(1));
    }

    #[test]
    fn pop_discards() {
        let prog = [Instr::Push(5), Instr::Pop, Instr::Push(3), Instr::Ret];
        assert_eq!(run(&prog), Outcome::Halt(3));
    }

    #[test]
    fn forward_branch_takes_true_path() {
        // if 5 < 10 { 1 } else { 0 }
        // indices: 0 push,1 push,2 jlt,3 push,4 jmp,5 push,6 ret
        let prog = [
            Instr::Push(5),
            Instr::Push(10),
            Instr::Jlt(5),
            Instr::Push(0),
            Instr::Jmp(6),
            Instr::Push(1),
            Instr::Ret,
        ];
        assert_eq!(run(&prog), Outcome::Halt(1));
        // false path: 10 < 5 is false -> 0
        let prog = [
            Instr::Push(10),
            Instr::Push(5),
            Instr::Jlt(5),
            Instr::Push(0),
            Instr::Jmp(6),
            Instr::Push(1),
            Instr::Ret,
        ];
        assert_eq!(run(&prog), Outcome::Halt(0));
    }

    #[test]
    fn backward_loop_sums_locals() {
        // i = 5; acc = 0; loop: acc += i; i -= 1; while (0 < i); return acc
        let prog = [
            Instr::Push(5),
            Instr::Store(0),
            Instr::Push(0),
            Instr::Store(1),
            Instr::Load(1), // pc 4 loop:
            Instr::Load(0),
            Instr::Add,
            Instr::Store(1),
            Instr::Load(0),
            Instr::Push(1),
            Instr::Sub,
            Instr::Store(0),
            Instr::Push(0),
            Instr::Load(0),
            Instr::Jlt(4),
            Instr::Load(1),
            Instr::Ret,
        ];
        assert_eq!(run(&prog), Outcome::Halt(15));
    }

    #[test]
    fn fall_off_end_yields_top_or_zero() {
        assert_eq!(run(&[Instr::Push(7)]), Outcome::Halt(7));
        assert_eq!(run(&[Instr::Push(7), Instr::Pop]), Outcome::Halt(0));
        assert_eq!(run(&[]), Outcome::Halt(0));
    }

    #[test]
    fn trap_stack_underflow() {
        assert_eq!(run(&[Instr::Pop]), Outcome::Trap(Trap::StackUnderflow));
        let prog = [Instr::Push(1), Instr::Add]; // add needs two values
        assert_eq!(run(&prog), Outcome::Trap(Trap::StackUnderflow));
    }

    #[test]
    fn trap_stack_overflow() {
        let mut prog = vec![Instr::Push(1); STACK_CAP + 1];
        prog.push(Instr::Ret);
        assert_eq!(run(&prog), Outcome::Trap(Trap::StackOverflow));
    }

    #[test]
    fn trap_bad_local_index() {
        let prog = [Instr::Push(1), Instr::Store(16), Instr::Ret];
        assert_eq!(run(&prog), Outcome::Trap(Trap::BadLocal(16)));
    }

    #[test]
    fn trap_bad_pc_on_out_of_range_jump() {
        let prog = [Instr::Jmp(5)];
        assert_eq!(run(&prog), Outcome::Trap(Trap::BadPc(5)));
    }

    #[test]
    fn jump_past_end_halts() {
        // jeq to prog.len() is legal and halts with the top of the stack
        let prog = [
            Instr::Push(7),
            Instr::Push(7),
            Instr::Push(7),
            Instr::Jeq(5),
            Instr::Ret,
        ];
        assert_eq!(run(&prog), Outcome::Halt(7));
        // not-taken: jeq consumes both operands, leaving the value below
        let prog = [
            Instr::Push(8),
            Instr::Push(7),
            Instr::Push(9),
            Instr::Jeq(5),
            Instr::Ret,
        ];
        assert_eq!(run(&prog), Outcome::Halt(8));
    }

    #[test]
    fn step_limit_catches_livelock() {
        let prog = [Instr::Jmp(0)];
        assert_eq!(Vm::new().run(&prog, 100), Outcome::StepLimit);
    }

    // Terminator classification: arrives with the optimizer (its client).
}
