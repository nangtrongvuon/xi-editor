extern crate ignore;

use ignore::Walk;
use std::cmp::max;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// An instance of quick open

// Idea: Quick open should save a tree hierarchy of the current opened file's root folder (considered the workspace folder)
// Suggestions are pooled and given from a fuzzy finding structure.
// Suggestions are scored similarly to Sublime's own quick open.
// Based heavily on FTS's fuzzy find code and junegunn's fzf.

const SCORE_MATCH: usize = 30;
const SCORE_GAP_START: usize = 15;
const SCORE_GAP_EXTENSION: usize = 10;

const BONUS_BOUNDARY: usize = SCORE_MATCH / 2;
const BONUS_SYMBOL: usize = SCORE_MATCH / 2;
const BONUS_CAMEL: usize = BONUS_BOUNDARY + SCORE_GAP_EXTENSION;
const BONUS_CONSECUTIVE: usize = SCORE_GAP_START + SCORE_GAP_EXTENSION;
const BONUS_FIRST_CHAR_MULTIPLIER: usize = 2;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FuzzyResult {
    result_name: String,
    score: usize,
    // The start and end indices of the result's match.
    match_start: usize,
    match_end: usize,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub(crate) enum CharacterClass {
    Lower,
    Upper,
    Number,
    Symbol,
}

pub(crate) struct QuickOpen {
    // The current quick open root.
    root: PathBuf,

    // All the items found in the workspace.
    workspace_items: Vec<PathBuf>,

    // Fuzzy find results, sorted descending by score.
    fuzzy_results_map: HashMap<String, (usize, usize, usize)>,
}

impl QuickOpen {
    pub fn new() -> QuickOpen {
        QuickOpen {
            root: PathBuf::new(),
            workspace_items: Vec::new(),
            fuzzy_results_map: HashMap::new(),
        }
    }

    pub(crate) fn initialize_workspace_matches(&mut self, folder: &Path) -> &Path {
        let mut parents = vec![];
        let mut new_root = folder.parent().unwrap_or(folder);

        while let Some(parent) = new_root.parent() {
            parents.push(parent);
            new_root = parent;
        }

        // We're looking for a folder with ".git" in order to use it as our root path.
        // If none is found, the root is this folder's parent.
        for parent in parents.into_iter() {
            if parent.join(".git").exists() {
                new_root = parent;
                break;
            }
        }

        if new_root != self.root {
            self.root = new_root.to_owned();
            Walk::new(self.root.as_path()).filter_map(|v| v.ok()).for_each(|x| {
                let path = x.into_path();
                if !self.workspace_items.contains(&path) {
                    self.workspace_items.push(path);
                }
            });
        }
        // TODO: remove when PRing
        eprintln!("chosen root: {:?}", self.root);
        self.root.as_path()
    }

    // Returns a list of fuzzy find results sorted by score.
    pub(crate) fn get_quick_open_results(&mut self) -> Vec<FuzzyResult> {
        let mut fuzzy_results: Vec<FuzzyResult> = Vec::new();

        for (result_name, (score, start_index, end_index)) in self.fuzzy_results_map.drain() {
            fuzzy_results.push(FuzzyResult {
                result_name,
                score,
                match_start: start_index,
                match_end: end_index
            })
        }

        // Sort by descending score
        fuzzy_results.sort_by(|a, b| b.score.cmp(&a.score));
        fuzzy_results
    }

    pub(crate) fn initiate_fuzzy_match(&mut self, query: &str) {
        let mut average_score;
        let mut total_score = 0;
        let mut result_count = 0;

        for item in &self.workspace_items {
            if let Some(item_file_name) = item.file_name() {
                let (score, start_index, end_index) =
                    self.fuzzy_match(query, item_file_name.to_str().unwrap_or(""));

                result_count += 1;
                total_score += score;
                average_score = total_score / result_count;

                match item.strip_prefix(&self.root) {
                    Ok(shortened_path) => {
                        if let Ok(path_string) =
                            shortened_path.to_owned().into_os_string().into_string()
                        {
                            if score >= average_score {
                                eprintln!("{:?}, {:?}", start_index, end_index);
                                self.fuzzy_results_map
                                    .insert(path_string, (score, start_index, end_index));
                            }   
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "Encountered error {:?} while fuzzy matching for path: {:?}",
                            e, &item
                        );
                    }
                }
            }
        }
    }

    // Returns a score based on how much alike `pattern` is to `text`, along with their match indices.
    fn fuzzy_match(&self, pattern: &str, text: &str) -> (usize, usize, usize) {
        if pattern.is_empty() {
            return (0, 0, 0);
        }

        let mut p_index = 0;
        let mut start_index = 0;
        let mut end_index = 0;

        let pattern_length = pattern.len();
        let text_length = text.len();

        for i in 0..text_length {
            let char_index = text_length - i - 1;
            let pattern_index = pattern_length - p_index - 1;

            if let (Some(current_char), Some(pattern_char)) =
                (text.chars().nth(char_index), pattern.chars().nth(pattern_index))
            {
                if current_char == pattern_char {
                    if start_index == 0 {
                        start_index = i;
                    }

                    p_index += 1;

                    if p_index == pattern_length {
                        end_index = i + 1;
                        break;
                    }
                }
            }
        }

        if start_index > 0 && end_index > 0 {
            p_index -= 1;
            for i in (start_index..end_index - 1).rev() {
                let second_text_index = text_length - i - 1;
                let second_p_index = pattern_length - p_index - 1;

                if let (Some(current_char), Some(pattern_char)) =
                    (text.chars().nth(second_text_index), pattern.chars().nth(second_p_index))
                {
                    if current_char == pattern_char {
                        p_index -= 1;
                        if p_index == 0 {
                            start_index = i;
                            break;
                        }
                    }
                }
            }

            let score = self.calculate_score(pattern, text, start_index, end_index);
            return (score, start_index, end_index);
        } else {
            (0, 0, 0)
        }
    }

    fn calculate_score(
        &self,
        pattern: &str,
        text: &str,
        start_index: usize,
        end_index: usize,
    ) -> usize {
        let mut pattern_index = 0;
        let mut score = 0;
        let mut in_gap = false;
        let mut consecutive = 0;
        let mut first_bonus = 0;
        let mut prev_class = CharacterClass::Symbol;

        let mut text_chars = text.chars();
        let mut pattern_chars = pattern.chars();

        if start_index > 0 {
            if let Some(prev_char) = text_chars.nth(start_index - 1) {
                prev_class = self.get_char_class(prev_char);
            }
        }

        for i in start_index..end_index {
            if let (Some(text_char), Some(pattern_char)) =
                (text_chars.nth(i), pattern_chars.nth(pattern_index))
            {
                let current_class = self.get_char_class(text_char);

                if text_char == pattern_char {
                    score += SCORE_MATCH;
                    let mut bonus = self.calculate_bonus(prev_class, current_class);

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
                        score += SCORE_GAP_EXTENSION;
                    } else {
                        score += SCORE_GAP_START;
                    }

                    in_gap = true;
                    consecutive = 0;
                    first_bonus = 0;
                }
            }
        }
        score
    }

    fn get_char_class(&self, character: char) -> CharacterClass {
        if character.is_ascii_alphanumeric() {
            if character.is_ascii_alphabetic() {
                if character.is_ascii_lowercase() {
                    CharacterClass::Lower
                } else {
                    CharacterClass::Upper
                }
            } else {
                CharacterClass::Number
            }
        } else {
            CharacterClass::Symbol
        }
    }

    /// Calculates bonus for different character types.
    fn calculate_bonus(
        &self,
        first_char_class: CharacterClass,
        second_char_class: CharacterClass,
    ) -> usize {
        // Case: fuzzy_find, where "_" precedes "f"
        if first_char_class == CharacterClass::Symbol && second_char_class != CharacterClass::Symbol
        {
            return BONUS_BOUNDARY;
        }
        // Case: camelCase, letter123
        else if first_char_class == CharacterClass::Lower
            && second_char_class == CharacterClass::Upper
            || first_char_class != CharacterClass::Number
                && second_char_class == CharacterClass::Number
        {
            return BONUS_CAMEL;
        }
        // Case: symbols
        else if second_char_class == CharacterClass::Symbol {
            return BONUS_SYMBOL;
        }
        // No bonus for remaining cases
        0
    }
}
