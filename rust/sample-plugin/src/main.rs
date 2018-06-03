// Copyright 2016 Google Inc. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! A sample plugin, intended as an illustration and a template for plugin
//! developers.

extern crate xi_plugin_lib;
extern crate xi_core_lib as xi_core;
extern crate xi_rope;

use std::path::Path;
use std::collections::HashMap;

use xi_core::ConfigTable;
use xi_core::ViewId;
use xi_rope::rope::RopeDelta;
use xi_rope::interval::Interval;
use xi_rope::delta::Builder as EditBuilder;
use xi_plugin_lib::{Plugin, ChunkCache, View, mainloop, Error};

/// A type that implements the `Plugin` trait, and interacts with xi-core.
///
/// Currently, this plugin has a single noteworthy behaviour,
/// intended to demonstrate how to edit a document; when the plugin is active,
/// and the user inserts an exclamation mark, the plugin will capitalize the
/// preceding word.
#[derive(Default)]
struct SamplePlugin {
    views: HashMap<ViewId, HashMap<char, usize>>
}

//NOTE: implementing the `Plugin` trait is the sole requirement of a plugin.
// For more documentation, see `rust/plugin-lib` in this repo.
impl Plugin for SamplePlugin {
    type Cache = ChunkCache;

    fn new_view(&mut self, view: &mut View<Self::Cache>) {
        eprintln!("new view {}", view.get_id());
        self.views.insert(view.get_id(), HashMap::new());
        // view.add_status_item("my_key", &format!("hello {}", self.0), "left");
    }

    fn did_close(&mut self, view: &View<Self::Cache>) {
        eprintln!("close view {}", view.get_id());
        self.views.remove(&view.get_id());
        // view.remove_status_item("my_key");
    }

    fn did_save(&mut self, view: &mut View<Self::Cache>, _old: Option<&Path>) {
        eprintln!("saved view {}", view.get_id());
    }

    fn config_changed(&mut self, _view: &mut View<Self::Cache>, _changes: &ConfigTable) {
    }

    fn update(&mut self, view: &mut View<Self::Cache>, delta: Option<&RopeDelta>,
              _edit_type: String, _author: String) {

        //NOTE: example simple conditional edit. If this delta is
        //an insert of a single '!', we capitalize the preceding word.
        if let Some(delta) = delta {
            let (iv, _) = delta.summary();
            let text: String = delta.as_simple_insert()
                .map(String::from)
                .unwrap_or_default();
            if text == "!" {
                let _ = self.capitalize_word(view, iv.end());
            } else {
                // update_status
                self.update_status(view, &text);
            }
        }
    }
}

impl SamplePlugin {
    /// Uppercases the word preceding `end_offset`.
    fn capitalize_word(&self, view: &mut View<ChunkCache>, end_offset: usize)
        -> Result<(), Error>
    {
        //NOTE: this makes it clear to me that we need a better API for edits
        let line_nb = view.line_of_offset(end_offset)?;
        let line_start = view.offset_of_line(line_nb)?;

        let mut cur_utf8_ix = 0;
        let mut word_start = 0;
        for c in view.get_line(line_nb)?.chars() {
            if c.is_whitespace() {
                word_start = cur_utf8_ix;
            }

            cur_utf8_ix += c.len_utf8();

            if line_start + cur_utf8_ix == end_offset {
                break;
            }
        }

        let new_text = view.get_line(line_nb)?[word_start..end_offset-line_start]
            .to_uppercase();
        let buf_size = view.get_buf_size();
        let mut builder = EditBuilder::new(buf_size);
        let iv = Interval::new_closed_open(line_start + word_start, end_offset);
        builder.replace(iv, new_text.into());
        view.edit(builder.build(), 0, false, true, "sample".into());
        Ok(())
    }

    fn update_status(&mut self, view: &mut View<ChunkCache>, text: &str) {
        if text.chars().count() < 1 {
            return;
        }
        let view_state = self.views.get_mut(&view.get_id()).unwrap();
        let chr = text.chars().next().unwrap();
        match chr {
            uppercase_letter @ 'A' ... 'Z' => {
                let lower = uppercase_letter.to_lowercase().next().unwrap();
                if view_state.remove(&lower).is_some() {
                    view.remove_status_item(&lower.to_string());
                }
            }
            letter @ 'a' ... 'z' => { 
                if !view_state.contains_key(&letter) {
                    view_state.insert(letter, 1);
                    if letter < 'l' {
                        view.add_status_item(&letter.to_string(), &format!("{:?}:1", &letter.to_string()), "left");
                    } else {
                        view.add_status_item(&letter.to_string(), &format!("{:?}:1", &letter.to_string()), "right");
                    }
                } else {
                    let letter_count = view_state.get_mut(&letter).unwrap();
                    *letter_count += 1;
                    view.update_status_item(&letter.to_string(), &format!("{:?}:{:?}", &letter.to_string(), letter_count));
                }
            },
            _ => (),
        }

    }
}

fn main() {
    let mut plugin = SamplePlugin::default();
    mainloop(&mut plugin).unwrap();
}
