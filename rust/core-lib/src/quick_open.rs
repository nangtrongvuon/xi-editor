extern crate walkdir;

use std::path::{Path, PathBuf};
use std::collections::{BTreeMap};
use walkdir::{DirEntry, WalkDir};

// An instance of quick open

// Idea: Quick open should save a tree hierarchy of the current opened file's root folder (considered the workspace folder)
// Suggestions are pooled and given from a fuzzy finding structure
// Suggestions are scored similarly to Sublime's own quick open.
// Based heavily on FTS's fuzzy find code.


pub struct QuickOpen {
	workspace_items: Vec<PathBuf>
}

impl QuickOpen {
	pub fn new() -> QuickOpen {
		QuickOpen {
			workspace_items: Vec::new(),
		} 
	}

	pub fn initialize_workspace_matches(&mut self, folder: &Path) {

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
			.for_each(|x| {
					let path = x.into_path();
					if !self.workspace_items.contains(&path) {
						self.workspace_items.push(path);
					}
				});

		eprintln!("{:?}", self.workspace_items);
	}

	// Returns true if every char in pattern is found in string
	fn fuzzy_match_simple(pattern: &str, string: &str) -> bool {
		let mut count = 0;
		let mut pattern_chars = pattern.chars();

		for chr in string.chars() {
			if let Some(str_char) = pattern_chars.next() {
				if chr.to_lowercase().next() == str_char.to_lowercase().next() {
					count += 1;
				}
			}
		}

		return count == pattern.len()
	}

	fn fuzzy_match(pattern: &str, string: &str) -> (bool, usize) {
		fn fuzzy_match_recursive(pattern: &str, string: &str, mut score: usize, original_string: &str, mut source_matches: Vec<usize>, mut matches: Vec<usize>, max_matches: usize, next_match: usize, mut recursion_count: usize, recursion_limit: usize) -> (bool, usize) {
			recursion_count += 1;
			if recursion_count >= recursion_limit {
				return (false, score)
			}

			let mut pattern_idx = 0;
			let mut recursive_match = false;
			let mut best_recursive_matches: Vec<usize> = Vec::new();
			let best_recursive_score = 0;

			let string_chars = string.chars();
			let mut pattern_chars = pattern.chars().peekable();

			let mut first_match = true;

			for chr in string_chars {
				// println!("{}", chr);
				if let Some(pattern_char) = pattern_chars.peek() {
					// println!("pattern: {}", pattern_char);
					if chr.to_lowercase().next() == pattern_char.to_lowercase().next() {
						// println!("match: {}", pattern_char);
						if next_match >= max_matches {
							return (false, score)
						}

						let mut recursive_matches: Vec<usize> = Vec::new();
						let recursive_score = 0;

						if first_match {
							matches.append(&mut source_matches);
							first_match = false
						}

						let (next_fuzzy, _) = fuzzy_match_recursive(pattern, &string[1..], recursive_score, original_string, matches.to_owned(), recursive_matches.to_owned(), max_matches, next_match, recursion_count, recursion_limit);

						if next_fuzzy {
							if !recursive_match || recursive_score > best_recursive_score {
								best_recursive_matches.append(&mut recursive_matches)
							}

							recursive_match = true;
						}

						pattern_idx += 1;
						pattern_chars.next();
						if string.len() > original_string.len() {
							matches.push(string.len() - original_string.len());
						} else {
							matches.push(original_string.len() - string.len());
						}
					}
				}
			}

			let matched = if pattern_idx == pattern.len() {
				true
			} else {
				false
			};

			if matched {
				let adjacent_bonus = 15;
				let separator_bonus = 30;
				let camel_bonus = 30;
				let first_letter_bonus = 15;

				let leading_letter_penalty = 5;
				let max_leading_letter_penalty = 15;
				let unmatched_letter_penalty = 1;

				score = 100;

				let mut penalty = leading_letter_penalty * matches[0];
				if penalty < max_leading_letter_penalty {
					penalty = max_leading_letter_penalty;
				}
				score -= penalty;

				let unmatched = string.len() - original_string.len() - next_match;
				score -= unmatched * unmatched_letter_penalty;

				for i in 0..next_match {
					let prev_idx;
					let current_idx = matches[i];
					if i > 0 {
						prev_idx = matches[i - 1];

						if current_idx == prev_idx + 1 {
							score += adjacent_bonus;
						}
					}

					if current_idx > 0 {
						let original_chars: Vec<char> = original_string.chars().collect();
						let neighbour = original_chars[current_idx - 1];
						let current = original_chars[current_idx];

						if neighbour.is_lowercase() && current.is_uppercase() {
							score += camel_bonus;
						}

						if neighbour == '_' || neighbour == '-' {
							score += separator_bonus;
						}

					} else {
						score += first_letter_bonus;
					}
				}

				if recursive_match && (!matched || best_recursive_score > score) {
					matches.append(&mut best_recursive_matches);
					score = best_recursive_score;
					return (true, score)
				} else if matched {	
					return (true, score)
				} else {
					return (false, 0)
				}
			}

			return (matched, score)
		}

		let recursion_count = 0;
		let recursion_limit = 10;

		return fuzzy_match_recursive(pattern, string, 0, pattern, Vec::new(), Vec::new(), 50, 0, recursion_count, recursion_limit)
	}
}