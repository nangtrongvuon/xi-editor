extern crate walkdir;

use std::path::Path;
use walkdir::{DirEntry, WalkDir};

// An instance of quick open
pub struct QuickOpen {

}

impl QuickOpen {
	pub fn new() -> QuickOpen {
		QuickOpen {} 
	}

	pub fn say_hello(&self, folder: &Path) {

		fn is_not_hidden(entry: &DirEntry) -> bool {
			entry.file_name()
				 .to_str()
				 .map(|s| entry.depth() == 0 || !s.starts_with("."))
				 .unwrap_or(false)
		}

		WalkDir::new(folder)
			.into_iter()
			.filter_entry(|e| is_not_hidden(e))
			.filter_map(|v| v.ok())
			.for_each(|x| eprintln!("{}", x.path().display()));
	}
}