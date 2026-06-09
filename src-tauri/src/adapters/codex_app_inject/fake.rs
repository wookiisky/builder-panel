//! 单测用 Fake injector。

use std::cell::RefCell;

use crate::domain::app_error::AppError;

use super::CodexAppInjector;

/// 单测用 fake injector：记录调用顺序，可注入失败。
pub struct FakeCodexAppInjector {
    calls: RefCell<Vec<FakeCall>>,
    fail_step: RefCell<Option<FakeStep>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FakeStep {
    WaitFrontmost,
    FocusInput,
    PasteAndReturn,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FakeCall {
    WaitFrontmost { timeout_ms: u64 },
    FocusInput,
    PasteAndReturn { prompt: String },
}

impl FakeCodexAppInjector {
    pub fn new() -> Self {
        Self {
            calls: RefCell::new(Vec::new()),
            fail_step: RefCell::new(None),
        }
    }

    pub fn fail_at(&self, step: FakeStep) {
        *self.fail_step.borrow_mut() = Some(step);
    }

    pub fn calls(&self) -> Vec<FakeCall> {
        self.calls.borrow().clone()
    }

    fn maybe_fail(&self, step: FakeStep) -> Result<(), AppError> {
        if self.fail_step.borrow().as_ref() == Some(&step) {
            return Err(super::inject_error(
                format!("fake failure at {:?}", step),
                "test".to_string(),
                None,
            ));
        }
        Ok(())
    }
}

impl Default for FakeCodexAppInjector {
    fn default() -> Self {
        Self::new()
    }
}

impl CodexAppInjector for FakeCodexAppInjector {
    fn wait_codex_app_frontmost(&self, timeout_ms: u64) -> Result<(), AppError> {
        self.calls
            .borrow_mut()
            .push(FakeCall::WaitFrontmost { timeout_ms });
        self.maybe_fail(FakeStep::WaitFrontmost)
    }

    fn focus_input_field(&self) -> Result<(), AppError> {
        self.calls.borrow_mut().push(FakeCall::FocusInput);
        self.maybe_fail(FakeStep::FocusInput)
    }

    fn paste_and_return(&self, prompt: &str) -> Result<(), AppError> {
        self.calls
            .borrow_mut()
            .push(FakeCall::PasteAndReturn {
                prompt: prompt.to_string(),
            });
        self.maybe_fail(FakeStep::PasteAndReturn)
    }
}
