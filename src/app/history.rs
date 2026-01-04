use crate::model::RequestId;

use super::reducer::{Action, reduce};
use super::state::AppModel;

const SNAPSHOT_INTERVAL: usize = 50;
const DEFAULT_HISTORY_MAX: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Focus {
    None,
    Request(RequestId),
}

impl Focus {
    pub fn from_selection(selection: Option<&RequestId>) -> Self {
        selection.map_or(Self::None, |id| Self::Request(id.clone()))
    }

    pub const fn request(&self) -> Option<&RequestId> {
        match self {
            Self::None => None,
            Self::Request(id) => Some(id),
        }
    }

    pub fn into_request(self) -> Option<RequestId> {
        match self {
            Self::None => None,
            Self::Request(id) => Some(id),
        }
    }
}

#[derive(Debug, Clone)]
struct HistoryEntry {
    action: Action,
    focus: Focus,
}

#[derive(Debug, Clone)]
struct Snapshot {
    index: usize,
    model: AppModel,
    focus: Focus,
}

#[derive(Debug)]
pub struct HistoryResult {
    pub model: AppModel,
    pub focus: Focus,
}

#[derive(Debug)]
pub struct History {
    entries: Vec<HistoryEntry>,
    cursor: usize,
    snapshots: Vec<Snapshot>,
    max: usize,
}

impl History {
    pub fn new(initial: AppModel) -> Self {
        Self {
            entries: Vec::new(),
            cursor: 0,
            snapshots: vec![Snapshot {
                index: 0,
                model: initial,
                focus: Focus::None,
            }],
            max: DEFAULT_HISTORY_MAX,
        }
    }

    pub fn record(&mut self, action: Action, focus: Focus, model: &AppModel) {
        if self.cursor < self.entries.len() {
            self.entries.truncate(self.cursor);
            self.snapshots.retain(|snap| snap.index <= self.cursor);
        }

        self.entries.push(HistoryEntry { action, focus: focus.clone() });
        self.cursor += 1;

        if self.cursor.is_multiple_of(SNAPSHOT_INTERVAL) {
            self.snapshots.push(Snapshot {
                index: self.cursor,
                model: model.clone(),
                focus,
            });
        }
    }

    pub fn undo<F>(&mut self, apply_focus: F) -> Option<HistoryResult>
    where
        F: Fn(&Focus, &mut AppModel),
    {
        if self.cursor == 0 {
            return None;
        }
        self.cursor -= 1;
        Some(self.replay_to(self.cursor, apply_focus))
    }

    pub fn redo<F>(&mut self, apply_focus: F) -> Option<HistoryResult>
    where
        F: Fn(&Focus, &mut AppModel),
    {
        if self.cursor >= self.entries.len() {
            return None;
        }
        self.cursor += 1;
        Some(self.replay_to(self.cursor, apply_focus))
    }

    pub fn trim_if_needed<F>(&mut self, apply_focus: F)
    where
        F: Fn(&Focus, &mut AppModel),
    {
        if self.entries.len() <= self.max {
            return;
        }

        let overflow = self.entries.len() - self.max;
        let base = self.replay_to(overflow, apply_focus);
        self.entries.drain(0..overflow);
        self.cursor = self.cursor.saturating_sub(overflow);
        self.snapshots.clear();
        self.snapshots.push(Snapshot {
            index: 0,
            model: base.model,
            focus: base.focus,
        });
    }

    fn replay_to<F>(&self, target: usize, apply_focus: F) -> HistoryResult
    where
        F: Fn(&Focus, &mut AppModel),
    {
        let snapshot = self
            .snapshots
            .iter()
            .filter(|snap| snap.index <= target)
            .max_by_key(|snap| snap.index)
            .expect("history snapshot");
        let mut model = snapshot.model.clone();
        let mut current_focus = snapshot.focus.clone();
        let mut last_focus = current_focus.clone();

        for entry in self.entries.iter().take(target).skip(snapshot.index) {
            if entry.focus != current_focus {
                current_focus = entry.focus.clone();
                apply_focus(&current_focus, &mut model);
            }
            let (next_model, _effects) = reduce(model, entry.action.clone());
            model = next_model;
            last_focus = current_focus.clone();
        }

        HistoryResult {
            model,
            focus: last_focus,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use crate::model::{RequestDraft, RequestId};

    use super::super::reducer::{Action};
    use super::super::state::{AppModel, LoadedDraft};
    use super::{Focus, History};

    fn apply_focus(
        drafts: &HashMap<RequestId, RequestDraft>,
        focus: &Focus,
        model: &mut AppModel,
    ) {
        let Some(id) = focus.request() else {
            return;
        };
        if let Some(draft) = drafts.get(id) {
            let loaded = LoadedDraft {
                draft: draft.clone(),
                save_path: "test.http".to_string(),
            };
            model.load_draft(loaded);
        }
    }

    #[test]
    fn history_replay_respects_focus() {
        let mut drafts = HashMap::new();
        let id_a = RequestId::HttpFile {
            path: PathBuf::from("a.http"),
            index: 0,
        };
        let id_b = RequestId::HttpFile {
            path: PathBuf::from("b.http"),
            index: 0,
        };
        drafts.insert(
            id_a.clone(),
            RequestDraft {
                title: "A".to_string(),
                ..Default::default()
            },
        );
        drafts.insert(
            id_b.clone(),
            RequestDraft {
                title: "B".to_string(),
                ..Default::default()
            },
        );

        let mut model = AppModel::default();
        let mut history = History::new(model.clone());

        apply_focus(&drafts, &Focus::Request(id_a.clone()), &mut model);
        let (model_after_a, _) = super::super::reducer::reduce(
            model,
            Action::TitleChanged("A1".to_string()),
        );
        model = model_after_a;
        history.record(Action::TitleChanged("A1".to_string()), Focus::Request(id_a.clone()), &model);

        apply_focus(&drafts, &Focus::Request(id_b.clone()), &mut model);
        let (model_after_b, _) = super::super::reducer::reduce(
            model,
            Action::TitleChanged("B1".to_string()),
        );
        model = model_after_b;
        history.record(Action::TitleChanged("B1".to_string()), Focus::Request(id_b.clone()), &model);

        let undo = history
            .undo(|focus, model| apply_focus(&drafts, focus, model))
            .expect("undo");
        assert_eq!(undo.focus, Focus::Request(id_a));
        assert_eq!(undo.model.draft.title, "A1");

        let redo = history
            .redo(|focus, model| apply_focus(&drafts, focus, model))
            .expect("redo");
        assert_eq!(redo.focus, Focus::Request(id_b));
        assert_eq!(redo.model.draft.title, "B1");
    }
}
