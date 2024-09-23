use core::default::Default;
use core::assert;
use spin::Mutex;

/// Max number of interrupt source, PLIC supports up-to 1023 interrupt sources(0 reserved)
static MAX_INTERRUPT_SOURCE_NUM: usize = 1024;

/// Max number of hart context, PLIC supports up-to 15872 contexts
static MAX_CONTEXT_NUM: usize = 15872;
static CONTEXT_NUM: usize = axconfig::SMP * 2;

/* Each interrupt source has a priority register associated with it. */
static PRIORITY_BASE: usize = 0;
static PRIORITY_PER_ID: usize = 4;

/*
 * Each hart context has a vector of interupt enable bits associated with it.
 * There's one bit for each interrupt source.
 */
static ENABLE_BASE: usize = 0x2000;
static ENABLE_PER_HART:usize = 0x80;

/*
 * Each hart context has a set of control registers associated with it.  Right
 * now there's only two: a source priority threshold over which the hart will
 * take an interrupt, and a register to claim interrupts.
 */
static CONTEXT_BASE: usize = 0x200000;
static CONTEXT_PER_HART: usize = 0x1000;
static CONTEXT_THRESHOLD: usize = 0;
static CONTEXT_CLAIM: usize = 4;


/// Virtual PLIC
struct PLIC {
    /// Base addr of plic
    base: usize,    
    
    /// Priority register of each interrupt source, each register is 4 Bytes.
    /// 1024 * 4 Bytes = 4096 Bytes in total
    interrupt_priority: [u32; MAX_INTERRUPT_SOURCE_NUM],       
    
    /// Pending bit of each interrupt source, each source has 1 pending bit
    /// 1024 * 1 Byte(size of rust bool) = 1024 Bytes in total
    // TODO: bitvec? 1024 * 1 bit / 8 bits = 128 Bytes in total
    interrupt_pending: [bool; MAX_INTERRUPT_SOURCE_NUM],
    // TODO: mutex?

    /// Each context has 1024 enable bits for each interrupt source
    /// CONTEX_NUM * 1024 * 1 Byte(size of rust bool) = 1024 * CONTEX_NUM Bytes in total
    // TODO: bitvec? CONTEX_NUM * 1024 * 1 bit / 8 bits = 128 * CONTEX_NUM Bytes in total
    interrupt_enable: [[bool; MAX_INTERRUPT_SOURCE_NUM]; CONTEXT_NUM],

    /// Priority threshold for each context, PLIC will mask all PLIC interrupts of a priority less than or equal to threshold
    // TODO: size? u32?
    priority_threshold: [u32; CONTEXT_NUM],

    /// Claim register for each context, as well as completion register
    /// TODO: size? u32?
    interrupt_claim: [u32; CONTEXT_NUM],


    // gate: Mutex<u8>,
}

impl Default for PLIC {
    fn default() -> Self {
        info!("vPLIC crated with CONTEXT_NUM {}", CONTEX_NUM);
        PLIC{
            base: 0x200_0000,
            interrupt_priority: [0, MAX_INTERRUPT_SOURCE_NUM],
            interrupt_pending: [false, MAX_INTERRUPT_SOURCE_NUM],
            interrupt_enable: [[false; MAX_INTERRUPT_SOURCE_NUM]; CONTEXT_NUM],
            priority_threshold: [0; CONTEXT_NUM],
            interrupt_claim: [0; CONTEXT_NUM],
        }
    }
}

impl PLIC {
    fn new(base: usize) -> Self {
        PLIC{
            base,
            ..Default::default()
        }
    }

    /// Read the priority of given interrupt source
    fn read_priority(&self, irq_source: u32) -> u32 {
        assert!(irq_source < MAX_INTERRUPT_SOURCE_NUM);
        let res = self.interrupt_priority[irq_source];
        debug!("vPLIC read irq <{}>'s priority <{}>", irq_source, res);
        res
    }

    // /// Read the pending bit of given interrupt source
    // fn read_pending(&self, irq_source: u32) -> bool {
    //     assert!(irq_source < MAX_INTERRUPT_SOURCE_NUM);
    //     let res = self.interrupt_pending[irq_source];
    //     debug!("vPLIC read irq <{}>'s pending <{}>", irq_source, res);
    //     res   
    // }

    /// Read the enable of given context and interrupt source
    fn read_enable(&self, context: u32, irq_source: u32) -> bool {
        assert!(context < MAX_CONTEXT_NUM);
        assert!(irq_source < MAX_INTERRUPT_SOURCE_NUM);
        let res = self.interrupt_enable[context][irq_source];
        debug!("vPLIC read context <{}> irq <{}>'s enable <{}>", context, irq_source, res);
        res
    }

    /// Read the threshold of given context
    fn read_threshold(&self, context: u32) -> u32 {
        assert!(context < MAX_CONTEXT_NUM);
        let res = self.priority_threshold[context];
        debug!("vPLIC read context <{}>'s threshold <{}>", context, res);
        res
    }




    /// Write the priority of given interrupt source
    fn write_priority(&mut self, irq_source: u32, priority: u32) {
        assert!(irq_source < MAX_INTERRUPT_SOURCE_NUM);
        let mut target = &self.interrupt_priority[irq_source];
        debug!("vPLIC write irq <{}>'s priority <{}> to <{}>", irq_source, target, priority);
        target = priority;
        assert!(self.interrupt_priority[irq_source] == priority);
    }

    // /// Write the pending bit of given interrupt source
    // fn write_pending(&self, irq_source: u32) -> bool {
    //     assert!(irq_source < MAX_INTERRUPT_SOURCE_NUM);
    //     let res = self.interrupt_pending[irq_source];
    //     debug!("vPLIC read irq <{}>'s pending <{}>", irq_source, res);
    //     res   
    // }

    /// Enable given interrupt source of given context
    fn enable(&self, context: u32, irq_source: u32) -> bool {
        assert!(context < MAX_CONTEXT_NUM);
        assert!(irq_source < MAX_INTERRUPT_SOURCE_NUM);
        let mut target = &self.interrupt_enable[context][irq_source];
        debug!("vPLIC enable context <{}> irq <{}> from {}", context, irq_source, target);
        target = true;
        assert!(self.interrupt_enable[context][irq_source] = true);
    }

    /// disable given interrupt source of given context
    fn disable(&self, context: u32, irq_source: u32) -> bool {
        assert!(context < MAX_CONTEXT_NUM);
        assert!(irq_source < MAX_INTERRUPT_SOURCE_NUM);
        let mut target = &self.interrupt_enable[context][irq_source];
        debug!("vPLIC disable context <{}> irq <{}> from {}", context, irq_source, target);
        target = false;
        assert!(self.interrupt_enable[context][irq_source] = false);
    }

    /// Write the threshold of given context
    fn write_threshold(&self, context: u32, threshold: u32) {
        assert!(context < MAX_CONTEXT_NUM);
        let mut target = &self.priority_threshold[context];
        debug!("vPLIC write context <{}>'s threshold <{}> to <{}>", context, target, threshold);
        target = threshold;
        assert!(self.priority_threshold[context] == threshold);
    }









    /// Do claim
    fn claim(&self, context: u32) -> u32 {
        assert!(context < MAX_CONTEXT_NUM);
        let res = self.interrupt_claim[context];
        debug!("vPLIC do context <{}>'s claim <{}>", context, res);
        res
    }

    





    fn read_u32(addr: usize) -> u32 {

    }
}
