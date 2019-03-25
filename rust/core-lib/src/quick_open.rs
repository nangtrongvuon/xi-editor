extern crate walkdir;

use std::path::{Path, PathBuf};
use std::cmp::{max};
use walkdir::{DirEntry, WalkDir};

// An instance of quick open

// Idea: Quick open should save a tree hierarchy of the current opened file's root folder (considered the workspace folder)
// Suggestions are pooled and given from a fuzzy finding structure
// Suggestions are scored similarly to Sublime's own quick open.
// Based heavily on FTS's fuzzy find code.

const SCORE_MATCH: usize = 16;
const SCORE_GAP_START: usize = 3;
const SCORE_GAP_EXTENSION: usize = 1;

const BONUS_BOUNDARY: usize = SCORE_MATCH / 2;
const BONUS_CAMEL: usize = BONUS_BOUNDARY + SCORE_GAP_EXTENSION;
const BONUS_CONSECUTIVE: usize = SCORE_GAP_START + SCORE_GAP_EXTENSION;
const BONUS_FIRST_CHAR_MULTIPLIER: usize = 2;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FuzzyResult {
	result_name: Option<String>,
	score: usize
}

pub struct QuickOpen {
	// All the items found in the workspace.
	workspace_items: Vec<PathBuf>,

	// Fuzzy find results, sorted descending by score.
	fuzzy_results: Vec<FuzzyResult>,
}

impl QuickOpen {
	pub fn new() -> QuickOpen {
		QuickOpen {
			workspace_items: Vec::new(),
			fuzzy_results: Vec::new(),
		} 
	}

	pub fn initialize_workspace_matches(&mut self, folder: &Path) {
		fn is_not_hidden(entry: &DirEntry) -> bool {
			entry.file_name()
				 .to_str()
				 .map(|s| entry.depth() == 0 || !s.starts_with('.'))
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

	pub fn initiate_fuzzy_match(&mut self, query: &str) -> Vec<FuzzyResult> {
		for item in &self.workspace_items {
			if let Some(item_path_str) = item.to_str() {
				let fuzzy_result = self.fuzzy_match(query, item_path_str);	
				self.fuzzy_results.push(fuzzy_result);	
			} 
		}

		self.fuzzy_results.sort_by(|a, b| b.score.cmp(&a.score));
		return self.fuzzy_results.clone()
	}

	// Returns true if every char in pattern is found in text
	fn fuzzy_match_simple(pattern: &str, text: &str) -> bool {
		let mut count = 0;
		let mut pattern_chars = pattern.chars();

		for chr in text.chars() {
			if let Some(str_char) = pattern_chars.next() {
				if chr.to_lowercase().next() == str_char.to_lowercase().next() {
					count += 1;
				}
			}
		}
		
		count == pattern.len()
	}

	fn fuzzy_match(&self, pattern: &str, text: &str) -> FuzzyResult {
		if pattern.len() == 0 {
			return FuzzyResult { result_name: None, score: 0 }
		}

		let mut p_index = 0;
		let mut start_index = 0;
		let mut end_index = 0;

		let pattern_length = pattern.len();
		let text_length = text.len();

		for i in 0..text_length {
			let char_index = text_length - i - 1;
			let pattern_index = pattern_length - p_index - 1;

			if let (Some(current_char), Some(pattern_char)) = (text.chars().nth(char_index), pattern.chars().nth(pattern_index)) {
				if current_char == pattern_char {
					if start_index == 0 {
						start_index = i
					}

					p_index = p_index + 1;

					if p_index == pattern_length {
						end_index = i + 1;
						break
					}
				}
			}
		}

		if start_index > 0 && end_index > 0 {

			for i in (start_index..end_index - 1).rev() {
				let second_text_index = text_length - i - 1;
				let second_p_index = pattern_length - p_index - 1;	

				if let (Some(current_char), Some(pattern_char)) = (text.chars().nth(second_text_index), pattern.chars().nth(second_p_index)) {
					if current_char == pattern_char {
						p_index = p_index - 1;
						if p_index == 0 {
							start_index = i;
							break
						}
					}
				}
			}

			let score = self.calculate_score(pattern, text, start_index, end_index);
			return FuzzyResult { result_name: Some(text.to_string()), score: score }

		} else {

			// start_index = text_length - end_index;
			// end_index = text_length - start_index;

			return FuzzyResult { result_name: None, score: 0 }
		}
	}

	fn calculate_score(&self, pattern: &str, text: &str, start_index: usize, end_index: usize) -> usize {

		let mut pattern_index = 0;
		let mut score = 0;
		let mut in_gap = false;
		let mut consecutive = 0;
		let mut first_bonus = 0;

		for i in start_index..end_index {
			if let (Some(text_char), Some(pattern_char)) = (text.chars().nth(i), pattern.chars().nth(pattern_index)) {
				if text_char == pattern_char {
					score = score + SCORE_MATCH;
					let mut bonus = {
						0
					};
					if consecutive == 0 {
						first_bonus = bonus;
					} else {
						if bonus == BONUS_BOUNDARY {
							first_bonus = bonus;
						}
						bonus = max(max(bonus, first_bonus), BONUS_CONSECUTIVE);
					}

					if pattern_index == 0 {
						score = score + bonus * BONUS_FIRST_CHAR_MULTIPLIER;
					} else {
						score = score + bonus;
					}

					in_gap = false;
					consecutive = consecutive + 1;
					pattern_index = pattern_index + 1;
				} else {
					if in_gap {
						score = score + SCORE_GAP_EXTENSION;
					} else {
						score = score + SCORE_GAP_START;
					}

					in_gap = true;
					consecutive = 0;
					first_bonus = 0;
				}
			}
		}

		return score
	}
}