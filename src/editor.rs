use crate::timeline::Clip;
use crate::effects::Effect;

pub struct VideoEditor {
    undo_stack: Vec<EditorAction>,
    redo_stack: Vec<EditorAction>,
}

#[derive(Debug, Clone)]
pub enum EditorAction {
    AddClip(Clip),
    RemoveClip(usize),
    MoveClip(usize, f32),
    ApplyEffect(usize, Effect),
    RemoveEffect(usize, usize),
}

impl VideoEditor {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn perform_action(&mut self, action: EditorAction) {
        self.undo_stack.push(action);
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) -> Option<EditorAction> {
        self.undo_stack.pop().map(|action| {
            self.redo_stack.push(action.clone());
            action
        })
    }

    pub fn redo(&mut self) -> Option<EditorAction> {
        self.redo_stack.pop().map(|action| {
            self.undo_stack.push(action.clone());
            action
        })
    }
}

impl Default for VideoEditor {
    fn default() -> Self {
        Self::new()
    }
}
