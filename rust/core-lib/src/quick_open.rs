extern crate walkdir;

use std::path::{Path, PathBuf};
use std::collections::{HashMap};
use std::cmp::{max};
use walkdir::{DirEntry, WalkDir};

// An instance of quick open

// Idea: Quick open should save a tree hierarchy of the current opened file's root folder (considered the workspace folder)
// Suggestions are pooled and given from a fuzzy finding structure
// Suggestions are scored similarly to Sublime's own quick open.
// Based heavily on FTS's fuzzy find code and junegunn's fzf.

const SCORE_MATCH: usize = 16;
const SCORE_GAP_START: usize = 3;
const SCORE_GAP_EXTENSION: usize = 1;

const BONUS_BOUNDARY: usize = SCORE_MATCH / 2;
const BONUS_SYMBOL: usize = SCORE_MATCH / 2;
const BONUS_CAMEL: usize = BONUS_BOUNDARY + SCORE_GAP_EXTENSION;
const BONUS_CONSECUTIVE: usize = SCORE_GAP_START + SCORE_GAP_EXTENSION;
const BONUS_FIRST_CHAR_MULTIPLIER: usize = 2;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FuzzyResult {
	result_name: String,
	score: usize
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub(crate) enum CharacterClass {
	CharLower,
	CharUpper,
	CharLetter,
	CharNumber,
	CharSymbol
}

pub(crate) struct QuickOpen {
	// All the items found in the workspace.
	workspace_items: Vec<PathBuf>,

	// Fuzzy find results, sorted descending by score.
	fuzzy_results_map: HashMap<String, usize>,
}

impl QuickOpen {
	pub fn new() -> QuickOpen {
		QuickOpen {
			workspace_items: Vec::new(),
			fuzzy_results_map: HashMap::new(),
		} 
	}

	pub(crate) fn initialize_workspace_matches(&mut self, folder: &Path) {
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
					if !self.workspace_items.contains(&path) && 
					path.extension().and_then(|p| p.to_str()).unwrap_or("") == "rs" {
						self.workspace_items.push(path);
					}
				});
	}

	// Returns a list of fuzzy find results sorted by score.
	pub(crate) fn get_quick_open_results(&mut self) -> Vec<FuzzyResult> {
		//TODO: Go through completions hashmap and convert to fuzzy results
		let fuzzy_results_iter = self.fuzzy_results_map.drain();
		let mut fuzzy_results = Vec::new();

		for (result_name, score) in fuzzy_results_iter {
			let new_result = FuzzyResult { result_name, score };
			fuzzy_results.push(new_result);
		}
		
		// Descending score
		fuzzy_results.sort_by(|a, b| b.score.cmp(&a.score));
		fuzzy_results
	}

	pub(crate) fn initiate_fuzzy_match(&mut self, query: &str) {

		let mut average_score;
		let mut total_score = 0;
		let mut result_count = 0;

		for item in &self.workspace_items {
			if let Some(item_file_name) = item.file_name() {
				let (result_name, score) = self.fuzzy_match(query, item_file_name.to_str().unwrap_or(""));	

				result_count += 1;
				total_score += score;
				average_score = total_score / result_count;

				if let Some(result_name) = result_name {	
					if score > average_score {
						self.fuzzy_results_map.insert(result_name, score);
					}
				}
			}
		}
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

	fn fuzzy_match(&self, pattern: &str, text: &str) -> (Option<String>, usize) {
		if pattern.is_empty() {
			return (None, 0)
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

					p_index += 1;

					if p_index == pattern_length {
						end_index = i + 1;
						break
					}
				}
			}
		}

		if start_index > 0 && end_index > 0 {
			p_index -= 1;
			for i in (start_index..end_index - 1).rev() {
				let second_text_index = text_length - i - 1;
				let second_p_index = pattern_length - p_index - 1;	

				if let (Some(current_char), Some(pattern_char)) = (text.chars().nth(second_text_index), pattern.chars().nth(second_p_index)) {
					if current_char == pattern_char {
						p_index -= 1;
						if p_index == 0 {
							start_index = i;
							break
						}
					}
				}
			}

			let score = self.calculate_score(pattern, text, start_index, end_index);
			(Some(text.to_string()), score)

		} else {
			(None, 0)
		}
	}

	fn calculate_score(&self, pattern: &str, text: &str, start_index: usize, end_index: usize) -> usize {

		let mut pattern_index = 0;
		let mut score = 0;
		let mut in_gap = false;
		let mut consecutive = 0;
		let mut first_bonus = 0;
		let mut prev_class = CharacterClass::CharSymbol;

		if start_index > 0 {	
			if let Some(prev_char) = text.chars().nth(start_index - 1) {
				prev_class = self.get_char_class(&prev_char);	
			}
		}

		for i in start_index..end_index {
			if let (Some(text_char), Some(pattern_char)) = (text.chars().nth(i), pattern.chars().nth(pattern_index)) {
				let current_class = self.get_char_class(&text_char);

				if text_char == pattern_char {
					score += SCORE_MATCH;
					let mut bonus = {
						self.calculate_bonus(prev_class, current_class)
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
						score += bonus * BONUS_FIRST_CHAR_MULTIPLIER;
					} else {
						score += bonus;
					}

					in_gap = false;
					consecutive += 1;
					pattern_index += 1;
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
		score
	}

	fn get_char_class(&self, character: &char) -> CharacterClass {
		if character.is_ascii_alphanumeric() {
			if character.is_ascii_alphabetic() {
				if character.is_ascii_lowercase() {
					return CharacterClass::CharLower
				} else {
					return CharacterClass::CharUpper
				}
			} else {
				return CharacterClass::CharNumber
			}
		} else {
			return CharacterClass::CharSymbol
		}
	}

	/// Calculates bonus for different character types.
	fn calculate_bonus(&self, first_char_class: CharacterClass, second_char_class: CharacterClass) -> usize {
		// Case: fuzzy_find, where "_" precedes "f"
		if first_char_class == CharacterClass::CharSymbol && second_char_class != CharacterClass::CharSymbol {
			return BONUS_BOUNDARY
		} 
		// Case: camelCase, letter123
		else if first_char_class == CharacterClass::CharLower && second_char_class == CharacterClass::CharUpper || first_char_class != CharacterClass::CharNumber && second_char_class == CharacterClass::CharNumber {
			return BONUS_CAMEL
		} 
		// Case: symbols
		else if second_char_class == CharacterClass::CharSymbol {
			return BONUS_SYMBOL
		}

		return 0
	}

}