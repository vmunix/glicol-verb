//! Parameter injection for Glicol code
//!
//! Prepends `~name: sig value` definitions for parameters referenced in user code.
//! This allows GUI sliders to control Glicol variables like ~drive, ~knob1, etc.

/// All injectable parameter names
pub const PARAM_NAMES: &[&str] = &[
    "knob1", "knob2", "knob3", "knob4", "drive", "feedback", "mix", "rate",
];

/// Parameter values for injection
#[derive(Default)]
pub struct ParamInjector {
    pub knob1: f32,
    pub knob2: f32,
    pub knob3: f32,
    pub knob4: f32,
    pub drive: f32,
    pub feedback: f32,
    pub mix: f32,
    pub rate: f32,
}

impl ParamInjector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Inject parameter definitions into user code
    ///
    /// Scans the code for `~name` references and prepends `~name: sig value`
    /// for each referenced parameter.
    ///
    /// Example:
    /// - User writes: `out: ~input >> mul ~drive`
    /// - With drive=2.0, becomes:
    ///   ```
    ///   ~drive: sig 2.0
    ///   out: ~input >> mul ~drive
    ///   ```
    pub fn inject(&self, user_code: &str) -> String {
        let mut injected_lines = Vec::new();

        // Check each parameter for references in the code
        for name in PARAM_NAMES {
            let reference = format!("~{}", name);
            if user_code.contains(&reference) {
                let value = self.get_value(name);
                injected_lines.push(format!("~{}: sig {:.6}", name, value));
            }
        }

        if injected_lines.is_empty() {
            user_code.to_string()
        } else {
            // Prepend injected definitions before user code
            format!("{}\n{}", injected_lines.join("\n"), user_code)
        }
    }

    /// Get the value of a parameter by name
    fn get_value(&self, name: &str) -> f32 {
        match name {
            "knob1" => self.knob1,
            "knob2" => self.knob2,
            "knob3" => self.knob3,
            "knob4" => self.knob4,
            "drive" => self.drive,
            "feedback" => self.feedback,
            "mix" => self.mix,
            "rate" => self.rate,
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inject_single_param() {
        let mut injector = ParamInjector::new();
        injector.drive = 2.5;

        let code = "out: ~input >> mul ~drive";
        let result = injector.inject(code);

        assert!(result.contains("~drive: sig 2.5"));
        assert!(result.ends_with(code));
    }

    #[test]
    fn test_inject_multiple_params() {
        let mut injector = ParamInjector::new();
        injector.drive = 2.0;
        injector.feedback = 0.5;

        let code = "out: ~input >> mul ~drive >> delay ~feedback";
        let result = injector.inject(code);

        assert!(result.contains("~drive: sig 2.0"));
        assert!(result.contains("~feedback: sig 0.5"));
    }

    #[test]
    fn test_no_injection_when_not_referenced() {
        let injector = ParamInjector::new();

        let code = "out: ~input >> mul 0.5";
        let result = injector.inject(code);

        // Should be unchanged
        assert_eq!(result, code);
    }

    #[test]
    fn test_knobs() {
        let mut injector = ParamInjector::new();
        injector.knob1 = 0.25;
        injector.knob2 = 0.75;

        let code = "out: ~input >> lpf ~knob1 ~knob2";
        let result = injector.inject(code);

        assert!(result.contains("~knob1: sig 0.25"));
        assert!(result.contains("~knob2: sig 0.75"));
    }
}
