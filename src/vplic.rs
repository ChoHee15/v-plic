use core::default::Default;
use core::assert;
use spin::mutex::Mutex;
use core::sync::atomic::{AtomicBool, Ordering};

extern crate alloc;
use alloc::vec::Vec;
use alloc::vec;
use alloc::sync::Arc;

use axlog::{info, debug, error};
use riscv::register::hvip;

// use arrayvec::ArrayVec;
use alloc::boxed::Box;

// use log::{debug, info};

// macro_rules! info {
//     // 匹配与 `println!` 类似的格式化字符串
//     ($($arg:tt)*) => {{
//         // 获取函数、文件、行号等信息
//         let location = core::panic::Location::caller();
//         let file = location.file();
//         let line = location.line();
//         // let function = location.function_name();
        
//         // 打印信息，带上所在文件、行号和函数名
//         // println!("[INFO] [{}:{}] [{}] {}", file, line, function, format_args!($($arg)*));
//         println!("[INFO] [{}:{}] {}", file, line, format_args!($($arg)*));
//     }}
// }

// macro_rules! debug {
//     // 匹配与 `println!` 类似的格式化字符串
//     ($($arg:tt)*) => {{
//         // 获取函数、文件、行号等信息
//         let location = core::panic::Location::caller();
//         let file = location.file();
//         let line = location.line();
//         // let function = location.function_name();
        
//         // 打印信息，带上所在文件、行号和函数名
//         // println!("[INFO] [{}:{}] [{}] {}", file, line, function, format_args!($($arg)*));
//         println!("[DEBUG] [{}:{}] {}", file, line, format_args!($($arg)*));
//     }}
// }

/// Max number of interrupt source, PLIC supports up-to 1023 interrupt sources(0 reserved)
const MAX_INTERRUPT_SOURCE_NUM: usize = 1024;
const INTERRUPT_SOURCE_NUM: usize = 1024;

/// Max number of hart context, PLIC supports up-to 15872 contexts
const MAX_CONTEXT_NUM: usize = 15872;
const CONTEXT_NUM: usize = axconfig::SMP * 2; // TODO: num of guest's vcpu

/* Each interrupt source has a priority register associated with it. */
const PRIORITY_BASE: usize = 0;
const PRIORITY_PER_ID: usize = 4;

/*
 * Each hart context has a vector of interupt enable bits associated with it.
 * There's one bit for each interrupt source.
 */
const ENABLE_BASE: usize = 0x2000;
const ENABLE_PER_HART:usize = 0x80;

/*
 * Each hart context has a set of control registers associated with it.  Right
 * now there's only two: a source priority threshold over which the hart will
 * take an interrupt, and a register to claim interrupts.
 */
const CONTEXT_BASE: usize = 0x200000;
const CONTEXT_PER_HART: usize = 0x1000;
const CONTEXT_THRESHOLD: usize = 0;
const CONTEXT_CLAIM: usize = 4;

use core::marker::Copy;

/// State and registers corresponding to Interrupt Source
#[derive(Debug)]
struct InterruptState {
    // Is the gateway available
    gateway: Mutex<bool>,
    // gateway: AtomicBool,

    /// Pending bit of each interrupt source
    interrupt_pending: AtomicBool,

    /// Priority register of each interrupt source, each register is 4 Bytes.
    // TODO: size? u32?
    interrupt_priority: u32,
}

impl Default for InterruptState {
    fn default() -> Self {
        InterruptState {
            gateway: Mutex::new(true),
            // gateway: AtomicBool::new(true),
            interrupt_pending: AtomicBool::new(false),
            interrupt_priority: 0,
        }
    }
}

/// Registers corresponding to HartContext
#[derive(Clone, Copy, Debug)]
struct HartContext {
    /// Each context has 1024 enable bits for each interrupt source
    interrupt_enable: [bool; INTERRUPT_SOURCE_NUM], //TODO: u32? bitvec?

    /// Priority threshold for each context, PLIC will mask all PLIC interrupts of a priority less than or equal to threshold
    // TODO: size? u32? bitvec?
    interrupt_threshold: u32,

    /// Claim register for each context, as well as completion register
    // TODO: size? u32?
    interrupt_claim: u32,
}

impl Default for HartContext {
    fn default() -> Self {
        HartContext {
            interrupt_enable: [false; INTERRUPT_SOURCE_NUM],
            interrupt_threshold: 0,
            interrupt_claim: 0,
        }
    }
}

/// Virtual PLIC
pub struct Plic {
    /// Base addr of plic
    base: usize,

    // interrupt_sources: [InterruptState; INTERRUPT_SOURCE_NUM],
    // interrupt_sources: ArrayVec<InterruptState, INTERRUPT_SOURCE_NUM>,
    // interrupt_sources: Box<[InterruptState; INTERRUPT_SOURCE_NUM]>,
    interrupt_sources: Vec<InterruptState>,

    // hart_contexts: [HartContext; CONTEXT_NUM],
    hart_contexts: Vec<HartContext>,
}

impl Default for Plic {
    fn default() -> Self {
        // info!("kkkkk");
        // info!("vPLIC crated with CONTEXT_NUM {}", CONTEXT_NUM);
        // info!("666");
        // let mut arr: ArrayVec<InterruptState, INTERRUPT_SOURCE_NUM> = ArrayVec::new();
        // for _ in 0..INTERRUPT_SOURCE_NUM {
        //     arr.push(InterruptState {
        //         gateway: Mutex::new(true),
        //         interrupt_pending: AtomicBool::new(false),
        //         interrupt_priority: 0,
        //     });
        // }
        // info!("qweqweqwe");
        let mut sources = Vec::with_capacity(INTERRUPT_SOURCE_NUM);
        for _ in 0..INTERRUPT_SOURCE_NUM {
            sources.push(InterruptState {
                gateway: Mutex::new(true),
                interrupt_pending: AtomicBool::new(false),
                interrupt_priority: 0,
            });
        }
        Plic{
            base: 0xC00_0000,
            // interrupt_sources: {
            //     let mut sources = Vec::with_capacity(INTERRUPT_SOURCE_NUM);
            //     for _ in 0..INTERRUPT_SOURCE_NUM {
            //         sources.push(InterruptState::default());
            //     }
            //     sources.try_into().unwrap_or_else(|v: Vec<_>| {
            //         panic!("Expected a Vec of length {}", INTERRUPT_SOURCE_NUM)
            //     })
            // },
            // interrupt_sources: arr, 
            // interrupt_sources: [InterruptState::default(); INTERRUPT_SOURCE_NUM],
            // interrupt_sources: Box::new([InterruptState {
            //     gateway: AtomicBool::new(true),
            //     interrupt_pending: AtomicBool::new(false),
            //     interrupt_priority: 0,
            // }; INTERRUPT_SOURCE_NUM]),
            interrupt_sources: sources,
            hart_contexts: vec![HartContext::default(); CONTEXT_NUM],
        }
    }
}

impl Plic {
    pub fn new(base: usize) -> Self {
        // info!("ereerer");
        Plic {
            base,
            ..Default::default()
        }
    }

    pub fn base(&self) -> usize {
        self.base
    }

    pub fn raise_interrupt(&mut self, irq_source_id: u32) {
        // Enter gateway
        assert!(irq_source_id < INTERRUPT_SOURCE_NUM as u32);
        let irq_source = &mut self.interrupt_sources[irq_source_id as usize];

        // TODO
        let mut cnt = 0;

        loop {
            let mut gate = irq_source.gateway.lock();
            if *gate  {
                info!("irq_source {} enter the gateway", irq_source_id);
                *gate = false;
                break;
            }
            info!("irq_source {} is blocked by the gateway!", irq_source_id);
            cnt += 1;
            drop(gate);
            if cnt == 10 {
                return;
            }
        }

        // Send request and latch it in IP
        irq_source.interrupt_pending.store(true, Ordering::SeqCst);
        

        // Notification
        self.notification();


        // Update

        
    }

    fn notification(&mut self) {
        for (ctx_id, ctx) in self.hart_contexts.iter_mut().enumerate() {
            let mut selected_prio: u32 = 0;
            let mut selected_irq: u32 = 0;
            for (irq_no, irq) in self.interrupt_sources.iter().enumerate() {
                // TODO: filter?
                if ctx.interrupt_enable[irq_no]
                    && irq.interrupt_pending.load(Ordering::SeqCst)
                    && irq.interrupt_priority > ctx.interrupt_threshold
                    && irq.interrupt_priority > selected_prio {

                        selected_irq = irq_no as u32;
                        selected_prio = irq.interrupt_priority;
                }
            }

            if selected_irq != 0 {
                ctx.interrupt_claim = selected_irq;
                // assert vcpu interrupt
                let hart_id = ctx_id / 2;
                info!("assert HART{} ctx {}'s irq!", hart_id, ctx_id);
                // todo!()
                // println!("set vcpu irq ing...");
                unsafe {
                    hvip::set_vseip();
                }
                
            } else {
              // deassert vcpu interrupt
            //   todo!()
                info!("deassert HART{} ctx {}'s irq", ctx_id / 2, ctx_id);

            }

        }
    }

    fn claim(&self, context_id: u32) -> u32{
        // read claim
        let claim = self.hart_contexts[context_id as usize].interrupt_claim;
        info!("Context {} of HART{} claim and get {}", context_id, context_id / 2, claim);
        // clear IP
        // assert!(self.interrupt_sources[claim as usize].interrupt_pending.load(Ordering::SeqCst) == true);
        self.interrupt_sources[claim as usize].interrupt_pending.store(false, Ordering::SeqCst);
        // irq_source.interrupt_pending = false;

        return claim;

    }

    fn complete(&mut self, context_id: u32, val: u32) {
        let ctx = &mut self.hart_contexts[context_id as usize];
        info!("Context{} of HART{} write complete from {} to {}", context_id, context_id / 2, ctx.interrupt_claim, val);
        // TODO: not necessary
        assert!(ctx.interrupt_claim == val);

        // write complete
        let irq_source = &mut self.interrupt_sources[ctx.interrupt_claim as usize];
        ctx.interrupt_claim = 0;

        // open gate
        let mut gate = irq_source.gateway.lock();
        assert!(*gate == false);
        *gate = true;

        // TODO
        error!("clean eip");
        unsafe {
            hvip::clear_vseip();
        }
    }

    pub fn read_u32(&self, addr: usize) -> u32 {
        let offset = addr.wrapping_sub(self.base);
        let res = match offset {
            0..=0xFFC => { // 0..0x1000 = prio
                // priority
                let irq_source = offset / 4;
                info!("read_u32 {:#x} priority of irq_source {}", offset, irq_source);
                assert!(irq_source < INTERRUPT_SOURCE_NUM);
                // read_priority
                // todo!();
                self.interrupt_sources[irq_source].interrupt_priority
            }
            0x1000..=0x107C => { // 0x1000..0x2000 = pending
                // pending
                // let irq_source = offset - 0x1000;
                // info!("read_u32 {:#x} pending of irq_source {}", offset, irq_source);
                panic!("pending is read only and unusual to use");
            }
            0x2000..=0x1F1FFC => { // 0x2000..0x20_0000 = enable
                // enable
                let ctx_id = (offset - ENABLE_BASE) / ENABLE_PER_HART;
                let inner_offset = offset - (ctx_id * ENABLE_PER_HART + ENABLE_BASE);
                let irq_word = inner_offset / 4;
                info!("read_u32 {:#x} enable of ctx {} with irq_word {}", offset, ctx_id, irq_word);
                // todo!();
                // self.hart_contexts[ctx_id].interrupt_enable
                let mut tmp: u32 = 0;
                for i in 0..32 {
                    let irq_no = irq_word * 32 + i;
                    tmp <<= 1;
                    tmp += self.hart_contexts[ctx_id].interrupt_enable[irq_no] as u32;
                    debug!("read enable {} bit {} of interrupt {} in context {}", i,
                        self.hart_contexts[ctx_id].interrupt_enable[irq_no], 
                        irq_no, ctx_id);
                }

                tmp
            }
            0x20_0000..=0x3FFF008 => { // x0x20_0000..0x400_0000  = context: threshold + claim
                // context
                let ctx_id = (offset - CONTEXT_BASE) / CONTEXT_PER_HART;
                let inner_offset = offset - (ctx_id * CONTEXT_PER_HART + CONTEXT_BASE);
                match inner_offset {
                    CONTEXT_THRESHOLD => { // threshold
                        // todo!();
                        info!("read_u32 {:#x} threshold of ctx{} with res {}", offset, ctx_id, 99999);
                        self.hart_contexts[ctx_id].interrupt_threshold
                    }
                    CONTEXT_CLAIM => { // claim
                        let claim = self.claim(ctx_id as u32);
                        info!("read_u32 {:#x} do claim of ctx{} with res {}", offset, ctx_id, claim);
                        claim
                    }
                    _ => {
                        panic!("unknown context offset");
                    }
                }
            }
            _ => {
                panic!("unknown plic region");
            }
        };

        info!("read_u32 res: {:#x} = {}D", res, res);
        res
    }

    pub fn write_u32(&mut self, addr: usize, val: u32) {
        let offset = addr.wrapping_sub(self.base);
        match offset {
            0..=0xFFC => { // 0..0x1000 = prio
                // priority
                let irq_source = offset / 4;
                info!("write_u32 {:#x} priority of irq_source {} to val {}", offset, irq_source, val);
                assert!(irq_source < INTERRUPT_SOURCE_NUM);
                // write_priority
                // todo!();
                self.interrupt_sources[irq_source].interrupt_priority = val;
                unsafe {
                    core::ptr::write_volatile(addr as *mut u32, val);
                }
                self.notification();

            }
            0x1000..=0x107C => { // 0x1000..0x2000 = pending
                panic!("pending is read only");
            }
            0x2000..=0x1F1FFC => { // 0x2000..0x20_0000 = enable
                // enable
                let ctx_id = (offset - ENABLE_BASE) / ENABLE_PER_HART;
                let inner_offset = offset - (ctx_id * ENABLE_PER_HART + ENABLE_BASE);
                let irq_word = inner_offset / 4;
                info!("write_u32 {:#x} enable of ctx {} with irq_word {} to val {:#x}", offset, ctx_id, irq_word, val);
                // todo!();
                for i in 0..32 {
                    let irq_no = irq_word * 32 + i;
                    let flag = if ((val >> i) & 0x1) == 0 {
                        false
                    }else{
                        true
                    };
                    self.hart_contexts[ctx_id].interrupt_enable[irq_no] = flag;
                    debug!("write enable {} bit {} of interrupt {} in context {}", i, flag, irq_no, ctx_id);
                }
                unsafe {
                    core::ptr::write_volatile(addr as *mut u32, val);
                }
                self.notification();
            }
            0x20_0000..=0x3FFF008 => { // x0x20_0000..0x400_0000 = context: threshold + claim
                // context
                let ctx_id = (offset - CONTEXT_BASE) / CONTEXT_PER_HART;
                let inner_offset = offset - (ctx_id * CONTEXT_PER_HART + CONTEXT_BASE);
                match inner_offset {
                    CONTEXT_THRESHOLD => { // threshold
                        info!("write_u32 {:#x} threshold of ctx {} with val {}", offset, ctx_id, val);
                        self.hart_contexts[ctx_id].interrupt_threshold = val;
                        unsafe {
                            core::ptr::write_volatile(addr as *mut u32, val);
                        }
                        self.notification();
                    }
                    CONTEXT_CLAIM => { // complete
                        info!("write_u32 {:#x} do complete of ctx {} with val {}", offset, ctx_id, val);
                        self.complete(ctx_id as u32, val);
                    }
                    _ => {
                        panic!("unknown context offset");
                    }
                }
            }
            _ => {
                panic!("unknown plic region, offset: {:#x}", offset);
            }
        }
        // Err(())
    }

}



