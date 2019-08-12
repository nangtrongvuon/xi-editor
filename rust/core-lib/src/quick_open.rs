extern crate ignore;

use ignore::Walk;
use std::path::{Path, PathBuf};

// An instance of quick open

// Idea: Quick open should save a tree hierarchy of the current opened file's root folder (considered the workspace folder)
// Suggestions are pooled and given from a fuzzy finding structure.
// Suggestions are scored similarly to Sublime's own quick open.
// Based heavily on FTS's fuzzy find code and junegunn's fzf.

// Prevents degenerate cases where matches are too long.
const MATCH_LIMIT: usize = 100;
const RECURSION_LIMIT: usize = 10;

const SEQUENTIAL_BONUS: usize = 16; // Bonus for adjacent matches
const SEPARATOR_BONUS: usize = 16; // Bonus for adjacent matches
const CAMELCASE_BONUS: usize = 16; // Bonus for adjacent matches
const FIRST_LETTER_BONUS: usize = 16; // Bonus for adjacent matches

const LEADING_LETTER_PENALTY: usize = 5; // Bonus for adjacent matches
const MAX_LEADING_LETTER_PENALTY: usize = 15; // Bonus for adjacent matches
const UNMATCHED_LETTER_PENALTY: usize = 1; // Bonus for adjacent matches

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FuzzyResult {
    result_name: String,
    score: usize,
    // All matching indices.
    match_indices: Vec<usize>,
}

pub(crate) struct QuickOpen {
    // The current quick open root.
    root: PathBuf,
    // All the items found in the workspace.
    workspace_items: Vec<PathBuf>,
    // Fuzzy find results, sorted descending by score.
    current_fuzzy_results: Vec<FuzzyResult>,
}

impl PartialEq for FuzzyResult {
    fn eq(&self, other: &Self) -> bool {
        self.result_name == other.result_name
    }
}

impl QuickOpen {
    pub fn new() -> QuickOpen {
        QuickOpen {
            root: PathBuf::new(),
            workspace_items: Vec::new(),
            current_fuzzy_results: Vec::new(),
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
            self.workspace_items.clear();
            self.root = new_root.to_owned();
            Walk::new(self.root.as_path()).filter_map(|v| v.ok()).for_each(|x| {
                let path = x.into_path();
                if !self.workspace_items.contains(&path) && path.is_file() {
                    self.workspace_items.push(path);
                }
            });
        }
        // TODO: remove when PRing
        eprintln!("Workspace items: {:?}", self.workspace_items);
        eprintln!("chosen root: {:?}", self.root);
        self.root.as_path()
    }

    // Returns a list of fuzzy find results sorted by score.
    pub(crate) fn get_quick_open_results(&mut self) -> &Vec<FuzzyResult> {
        self.current_fuzzy_results.sort_by(|a, b| b.score.cmp(&a.score));
        // self.current_fuzzy_results.dedup();
        return &self.current_fuzzy_results;
    }

    // Initiates a new fuzzy match session.
    pub(crate) fn initiate_fuzzy_match(&mut self, query: &str) {
        self.current_fuzzy_results.clear();
        for item in &self.workspace_items {
            if let Some(item_name) =
                item.file_name().map(|file_name| file_name.to_str().unwrap_or_default())
            {
                let (result_indices, result_score) =
                    self.fuzzy_match(query, item_name, None, Vec::new(), 0, 0, 0, 0);

                if result_indices.is_empty() {
                    continue;
                }

                match item.strip_prefix(&self.root) {
                    Ok(shortened_path) => {
                        if let Ok(path_string) =
                            shortened_path.to_owned().into_os_string().into_string()
                        {
                            // Shorten path here
                            let fuzzy_result = FuzzyResult {
                                result_name: path_string,
                                score: result_score,
                                match_indices: result_indices,
                            };

                            if !self.current_fuzzy_results.contains(&fuzzy_result) {
                                self.current_fuzzy_results.push(fuzzy_result);
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

    // Calculates how much alike `pattern` is to `text`, along with their match indices.
    // Algorithm ripped straight from FTS's fuzzy find blog post.
    // Returns a tuple containing if a match was found, and how much score is that match worth.
    fn fuzzy_match(
        &self,
        pattern: &str,
        text: &str,
        original_match_indices: Option<&Vec<usize>>,
        mut match_indices: Vec<usize>,
        mut pattern_current_idx: usize,
        mut text_current_idx: usize,
        mut matched_count: usize,
        mut recursion_count: usize,
    ) -> (Vec<usize>, usize) {
        let mut pattern_characters = pattern.chars();
        let mut text_characters = text.chars();

        eprintln!("Matching {:?} against {:?} with current recursion_count: {:?}", pattern, text, recursion_count);

        // Base case: pattern is empty
        recursion_count += 1;
        if recursion_count >= RECURSION_LIMIT || pattern.is_empty() {
            return (vec![], 0);
        }

        let mut score: usize = 0;
        let mut best_recursive_score: usize = 0;
        let mut best_recursive_match_indices: Vec<usize> = Vec::new();
        let mut first_match = true;
        let mut recursive_matched = false;

        while let (Some(pat_char), Some(text_char)) =
            (pattern_characters.next(), text_characters.next())
        {
            if pat_char.to_ascii_lowercase() == text_char.to_ascii_lowercase() {
                if matched_count >= MATCH_LIMIT {
                    return (vec![], 0);
                }

                if first_match {
                    if let Some(original_match_indices) = original_match_indices {
                        // eprintln!("Copying first match");
                        match_indices = original_match_indices[0..matched_count].to_vec();
                        first_match = false;
                    }
                }

                let recursive_matches: Vec<usize> = Vec::new();

                let (recursive_match_indices, recursive_score) = self.fuzzy_match(
                    pattern,
                    &text[1..],
                    Some(&match_indices),
                    recursive_matches,
                    pattern_current_idx,
                    text_current_idx + 1,
                    matched_count,
                    recursion_count,
                );

                if recursive_score > best_recursive_score {
                    best_recursive_match_indices = recursive_match_indices;
                    best_recursive_score = recursive_score;
                    recursive_matched = true;
                }

                match_indices.push(text_current_idx);
                matched_count += 1;
                pattern_current_idx += 1;
            }
            text_current_idx += 1;
        }

        let matched = pattern_current_idx == pattern.len();

        if matched {
            score = self.calculate_score(text, matched_count, &match_indices);
        }

        // If an answer from a further recursion is better
        if recursive_matched && (!matched || best_recursive_score > score) {
            // eprintln!("Copying recursive match");
            match_indices = best_recursive_match_indices;
            score = best_recursive_score;
            return (match_indices, score);
        } else if matched {
            return (match_indices, score);
        } else {
            return (vec![], 0);
        }
    }

    // Calculate a score, given a list of matched indices and the original text that matched.
    fn calculate_score(
        &self,
        text: &str,
        matched_count: usize,
        match_indices: &Vec<usize>,
    ) -> usize {
        // eprintln!("Calculating score");

        // Starting score
        let mut score: usize = 100;

        // Check if match didn't start from the first letter
        let mut penalty = LEADING_LETTER_PENALTY * match_indices[0];
        if penalty > MAX_LEADING_LETTER_PENALTY {
            penalty = MAX_LEADING_LETTER_PENALTY;
        }
        score = score.saturating_sub(penalty);

        // Apply penalty for non-matches
        let unmatched_penalty = match_indices[0] * UNMATCHED_LETTER_PENALTY;
        score = score.saturating_sub(unmatched_penalty);

        let mut previous_match_index: usize = 0;
        for i in 0..matched_count {
            let current_match_index = match_indices[i];

            if i > 0 {
                previous_match_index = match_indices[i - 1];
            }

            // Check for sequential matches
            if current_match_index == (previous_match_index + 1) {
                score += SEQUENTIAL_BONUS;
            }

            if current_match_index > 0 {
                match (
                    text.chars().nth(current_match_index - 1),
                    text.chars().nth(current_match_index),
                ) {
                    (Some(neighbour), Some(current_char)) => {
                        if neighbour.is_lowercase() && current_char.is_uppercase() {
                            score += CAMELCASE_BONUS;
                        }
                        if neighbour.to_string() == "_" || neighbour.to_string() == "-" {
                            score += SEPARATOR_BONUS;
                        }
                    }
                    _ => break,
                }
            } else {
                score += FIRST_LETTER_BONUS;
            }
        }
        return score;
    }
}
