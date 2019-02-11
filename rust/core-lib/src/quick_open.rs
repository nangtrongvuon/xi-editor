// An instance of quick open
pub struct QuickOpen {

}

impl QuickOpen {
	pub fn new() -> QuickOpen {
		QuickOpen {} 
	}

	pub fn say_hello(&self) {
		eprintln!("Hello world!");
	}
}