use solidus_engine::{ArgumentMode, ParseResult};

use crate::types::*;

pub fn assert_ids_sequential(result: &ParseResult) {
    for (i, cmd) in result.commands.iter().enumerate() {
        assert_eq!(cmd.id, format!("cmd-{i}"));
    }
    for (i, tb) in result.textblocks.iter().enumerate() {
        assert_eq!(tb.id, format!("text-{i}"));
    }
}

pub fn assert_argument_modes(result: &ParseResult) {
    for cmd in &result.commands {
        assert!(
            cmd.arguments.mode == ArgumentMode::SingleLine
                || cmd.arguments.mode == ArgumentMode::Fence
        );
    }
}

pub fn assert_unclosed_fence_warning(doc: &FuzzDoc, result: &ParseResult) {
    let rendered: Vec<_> = doc.fragments.iter().take(MAX_FRAGMENTS).collect();

    let has_any_unclosed = rendered
        .iter()
        .any(|f| matches!(f, Fragment::UnclosedFence(..)));

    if !has_any_unclosed {
        assert!(
            !result.warnings.iter().any(|w| w.wtype == "unclosed_fence"),
            "no UnclosedFence fragments but got unclosed_fence warning"
        );
        return;
    }

    let last_is_unclosed = rendered
        .last()
        .is_some_and(|f| matches!(f, Fragment::UnclosedFence(..)));

    if last_is_unclosed {
        assert!(
            result.warnings.iter().any(|w| w.wtype == "unclosed_fence"),
            "last fragment is UnclosedFence but no unclosed_fence warning emitted"
        );
    }
}