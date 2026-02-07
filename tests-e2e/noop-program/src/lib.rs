#![no_std]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn entrypoint(_input: *mut u8) -> u64 {
    0 // SUCCESS
}
