// #![no_std]

mod vplic;

use vplic::Plic;



fn main() {
    let mut plic = Plic::new(0xC00_0000);

    plic.write_u32(0xC00_0004, 5);
    
    plic.write_u32(0xC00_2000, 0x0000_0002);

    plic.raise_interrupt(1);
    let res = plic.read_u32(0xC20_0004);

    println!("res: {}", res);
    
    
    println!("Hello, world!");
}
