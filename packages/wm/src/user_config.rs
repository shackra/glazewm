use std::{collections::HashMap, env, fs, path::PathBuf};

use anyhow::{Context, Result};
use wm_common::{
  InvokeCommand, KeybindingConfig, MatchType, ParsedConfig,
  WindowMatchConfig, WindowRuleConfig, WindowRuleEvent, WorkspaceConfig,
};

use crate::{
  models::{Monitor, WindowContainer, Workspace},
  traits::{CommonGetters, WindowGetters},
};

/// Resource string for the sample config file.
const SAMPLE_CONFIG: &str =
  include_str!("../../../resources/assets/sample-config.yaml");

#[derive(Debug)]
pub struct UserConfig {
  /// Path to the user config file.
  pub path: PathBuf,

  /// Parsed user config value.
  pub value: ParsedConfig,

  /// Unparsed user config string.
  pub value_str: String,

  /// Hashmap of window rule event types (e.g. `WindowRuleEvent::Manage`)
  /// and the corresponding window rules of that type.
  window_rules_by_event: HashMap<WindowRuleEvent, Vec<WindowRuleConfig>>,

  /// Merged list of user-defined and default manage overrides. Windows
  /// matching these entries bypass Win32 style checks in
  /// `check_is_manageable()`.
  #[cfg(target_os = "windows")]
  pub manage_overrides: Vec<WindowMatchConfig>,
}

impl UserConfig {
  /// Creates an instance of `UserConfig`. Reads and validates the user
  /// config from the given path.
  ///
  /// Creates a new config file from sample if it doesn't exist.
  pub fn new(config_path: Option<PathBuf>) -> anyhow::Result<Self> {
    let default_config_path = home::home_dir()
      .context("Unable to get home directory.")?
      .join(".glzr/glazewm/config.yaml");

    let config_path = config_path
      .or_else(|| env::var("GLAZEWM_CONFIG_PATH").ok().map(PathBuf::from))
      .unwrap_or(default_config_path);

    let (config_value, config_str) = Self::read(&config_path)?;

    let window_rules_by_event = Self::window_rules_by_event(&config_value);

    Ok(Self {
      path: config_path,
      #[cfg(target_os = "windows")]
      manage_overrides: Self::merged_manage_overrides(&config_value),
      value: config_value,
      value_str: config_str,
      window_rules_by_event,
    })
  }

  /// Reads and validates the user config from the given path.
  ///
  /// Creates a new config file from sample if it doesn't exist.
  fn read(
    config_path: &PathBuf,
  ) -> anyhow::Result<(ParsedConfig, String)> {
    if !config_path.exists() {
      Self::create_sample(config_path)?;
    }

    let config_str = fs::read_to_string(config_path)
      .context("Unable to read config file.")?;

    // TODO: Improve error formatting of serde_yaml errors. Something
    // similar to https://github.com/AlexanderThaller/format_serde_error
    let config_value = serde_yaml::from_str(&config_str)?;

    Ok((config_value, config_str))
  }

  /// Initializes a new config file from the sample config resource.
  fn create_sample(config_path: &PathBuf) -> Result<()> {
    let parent_dir =
      config_path.parent().context("Invalid config path.")?;

    fs::create_dir_all(parent_dir).with_context(|| {
      format!("Unable to create directory {}.", &config_path.display())
    })?;

    fs::write(config_path, SAMPLE_CONFIG).with_context(|| {
      format!("Unable to write to {}.", config_path.display())
    })?;

    Ok(())
  }

  pub fn reload(&mut self) -> anyhow::Result<()> {
    let (config_value, config_str) = Self::read(&self.path)?;

    self.window_rules_by_event =
      Self::window_rules_by_event(&config_value);
    #[cfg(target_os = "windows")]
    {
      self.manage_overrides = Self::merged_manage_overrides(&config_value);
    }
    self.value = config_value;
    self.value_str = config_str;

    Ok(())
  }

  fn default_window_rules(
    config_value: &ParsedConfig,
  ) -> Vec<WindowRuleConfig> {
    let mut window_rules = Vec::new();

    let floating_defaults =
      &config_value.window_behavior.state_defaults.floating;

    // Default float rules.
    window_rules.push(WindowRuleConfig {
      commands: vec![InvokeCommand::SetFloating {
        centered: Some(floating_defaults.centered),
        shown_on_top: Some(floating_defaults.shown_on_top),
        x_pos: None,
        y_pos: None,
        width: None,
        height: None,
      }],
      match_window: vec![
        WindowMatchConfig {
          window_class: Some(MatchType::Equals { equals:
          // W10/W11 system dialog shown when moving and deleting files.
          "OperationStatusWindow".to_string(),
        }),
          ..WindowMatchConfig::default()
        },
        WindowMatchConfig {
          window_class: Some(MatchType::Equals { equals:
          // W10/W11 system dialogs (e.g. File Explorer save/open dialog).
          "#32770".to_string(),
        }),
          ..WindowMatchConfig::default()
        },
      ],
      on: vec![WindowRuleEvent::Manage],
      run_once: true,
    });

    // Default ignore rules.
    window_rules.push(WindowRuleConfig {
      commands: vec![InvokeCommand::Ignore],
      match_window: vec![
        WindowMatchConfig {
          window_process: Some(MatchType::Equals {
            equals: "SearchApp".to_string(),
          }),
          ..WindowMatchConfig::default()
        },
        WindowMatchConfig {
          window_process: Some(MatchType::Equals {
            equals: "SearchHost".to_string(),
          }),
          ..WindowMatchConfig::default()
        },
        WindowMatchConfig {
          window_process: Some(MatchType::Equals {
            equals: "ShellExperienceHost".to_string(),
          }),
          ..WindowMatchConfig::default()
        },
        WindowMatchConfig {
          window_process: Some(MatchType::Equals {
            // W10/11 start menu.
            equals: "StartMenuExperienceHost".to_string(),
          }),
          ..WindowMatchConfig::default()
        },
        WindowMatchConfig {
          window_process: Some(MatchType::Equals {
            // W10/11 screen snipping tool.
            equals: "ScreenClippingHost".to_string(),
          }),
          ..WindowMatchConfig::default()
        },
        WindowMatchConfig {
          window_process: Some(MatchType::Equals {
            // W11 lock screen.
            equals: "LockApp".to_string(),
          }),
          ..WindowMatchConfig::default()
        },
        WindowMatchConfig {
          window_class: Some(MatchType::Equals {
            // WSLg I/O helper window spawned by mstsc.exe on W11.
            equals: "OPContainerClass".to_string(),
          }),
          ..WindowMatchConfig::default()
        },
        WindowMatchConfig {
          window_class: Some(MatchType::Equals {
            // WSLg I/O helper window spawned by mstsc.exe on W11.
            equals: "IHWindowClass".to_string(),
          }),
          ..WindowMatchConfig::default()
        },
      ],
      on: vec![WindowRuleEvent::Manage],
      run_once: true,
    });

    window_rules
  }

  fn window_rules_by_event(
    config_value: &ParsedConfig,
  ) -> HashMap<WindowRuleEvent, Vec<WindowRuleConfig>> {
    let mut window_rules_by_event = HashMap::new();

    // Combine user-defined window rules with the default ones.
    let default_window_rules = Self::default_window_rules(config_value);
    let all_window_rules = config_value
      .window_rules
      .iter()
      .chain(default_window_rules.iter());

    for window_rule in all_window_rules {
      for event_type in &window_rule.on {
        window_rules_by_event
          .entry(event_type.clone())
          .or_insert_with(Vec::new)
          .push(window_rule.clone());
      }
    }

    window_rules_by_event
  }

  /// Returns the default manage overrides for known WSL2/WSLg X server
  /// processes and other applications that lack standard window styles.
  #[cfg(target_os = "windows")]
  fn default_manage_overrides() -> Vec<WindowMatchConfig> {
    vec![
      // Flow Launcher lacks standard window styles.
      WindowMatchConfig {
        window_process: Some(MatchType::Equals {
          equals: "Flow.Launcher".to_string(),
        }),
        window_title: Some(MatchType::Equals {
          equals: "Flow.Launcher".to_string(),
        }),
        ..WindowMatchConfig::default()
      },
      // WSL2 GUI via X410.
      WindowMatchConfig {
        window_process: Some(MatchType::Equals {
          equals: "X410".to_string(),
        }),
        ..WindowMatchConfig::default()
      },
      // WSL2 GUI via VcXsrv.
      WindowMatchConfig {
        window_process: Some(MatchType::Equals {
          equals: "vcxsrv".to_string(),
        }),
        ..WindowMatchConfig::default()
      },
      // WSLg on Windows 11 (built-in RDP client).
      WindowMatchConfig {
        window_process: Some(MatchType::Equals {
          equals: "msrdc".to_string(),
        }),
        ..WindowMatchConfig::default()
      },
    ]
  }

  /// Merges user-defined manage overrides with the built-in defaults.
  /// Entries where all match fields are `None` are excluded to prevent
  /// accidentally matching all windows.
  #[cfg(target_os = "windows")]
  fn merged_manage_overrides(
    config_value: &ParsedConfig,
  ) -> Vec<WindowMatchConfig> {
    let has_match_criteria = |m: &WindowMatchConfig| {
      m.window_process.is_some()
        || m.window_class.is_some()
        || m.window_title.is_some()
    };

    config_value
      .manage_overrides
      .iter()
      .filter(|m| has_match_criteria(m))
      .cloned()
      .chain(Self::default_manage_overrides())
      .collect()
  }

  /// Whether the window matches any manage override entry. Windows that
  /// match bypass standard Win32 style checks and may need special
  /// handling during repositioning (e.g. adjusted `SetWindowPos` flags).
  #[cfg(target_os = "windows")]
  pub fn is_manage_override(&self, window: &WindowContainer) -> bool {
    let props = window.native_properties();
    self.manage_overrides.iter().any(|m| {
      m.window_process
        .as_ref()
        .is_none_or(|p| p.is_match(&props.process_name))
        && m.window_class
          .as_ref()
          .is_none_or(|c| c.is_match(&props.class_name))
        && m.window_title
          .as_ref()
          .is_none_or(|t| t.is_match(&props.title))
    })
  }

  /// Window rules that should be applied to the window when the given
  /// event occurs.
  pub fn pending_window_rules(
    &self,
    window: &WindowContainer,
    event: &WindowRuleEvent,
  ) -> Vec<WindowRuleConfig> {
    let window_title = window.native_properties().title;
    #[cfg(target_os = "windows")]
    let window_class = window.native_properties().class_name;
    let window_process = window.native_properties().process_name;

    let pending_window_rules = self
      .window_rules_by_event
      .get(event)
      .unwrap_or(&Vec::new())
      .iter()
      .filter(|rule| {
        // Skip if window has already ran the rule.
        if window.done_window_rules().contains(rule) {
          return false;
        }

        // Check if the window matches the rule.
        rule.match_window.iter().any(|match_config| {
          let is_process_match = match_config
            .window_process
            .as_ref()
            .is_none_or(|match_type| {
              // TODO: Temp fix for matching Zebar on both platforms with
              // the same process name. Consider using lowercase for every
              // `equals` match type.
              if window_process == "Zebar" {
                match_type.is_match("Zebar")
                  || match_type.is_match("zebar")
              } else {
                match_type.is_match(&window_process)
              }
            });

          let is_class_match = {
            #[cfg(target_os = "windows")]
            {
              match_config.window_class.as_ref().is_none_or(|match_type| {
                match_type.is_match(&window_class)
              })
            }
            #[cfg(not(target_os = "windows"))]
            {
              match_config.window_class.is_none()
            }
          };

          let is_title_match = match_config
            .window_title
            .as_ref()
            .is_none_or(|match_type| match_type.is_match(&window_title));

          is_process_match && is_class_match && is_title_match
        })
      })
      .cloned()
      .collect::<Vec<_>>();

    pending_window_rules
  }

  pub fn inactive_workspace_configs(
    &self,
    active_workspaces: &[Workspace],
  ) -> Vec<&WorkspaceConfig> {
    self
      .value
      .workspaces
      .iter()
      .filter(|config| {
        !active_workspaces
          .iter()
          .any(|workspace| workspace.config().name == config.name)
      })
      .collect()
  }

  pub fn workspace_config_for_monitor(
    &self,
    monitor: &Monitor,
    active_workspaces: &[Workspace],
  ) -> Option<&WorkspaceConfig> {
    let inactive_configs =
      self.inactive_workspace_configs(active_workspaces);

    inactive_configs.into_iter().find(|&config| {
      config
        .bind_to_monitor
        .as_ref()
        .is_some_and(|monitor_index| {
          monitor.index() == *monitor_index as usize
        })
    })
  }

  /// Gets the first inactive workspace config, prioritizing configs that
  /// don't have a monitor binding.
  pub fn next_inactive_workspace_config(
    &self,
    active_workspaces: &[Workspace],
  ) -> Option<&WorkspaceConfig> {
    let inactive_configs =
      self.inactive_workspace_configs(active_workspaces);

    inactive_configs
      .iter()
      .find(|config| config.bind_to_monitor.is_none())
      .or(inactive_configs.first())
      .copied()
  }

  pub fn workspace_config_index(
    &self,
    workspace_name: &str,
  ) -> Option<usize> {
    self
      .value
      .workspaces
      .iter()
      .position(|config| config.name == workspace_name)
  }

  pub fn sort_workspaces(&self, workspaces: &mut [Workspace]) {
    workspaces.sort_by_key(|workspace| {
      self.workspace_config_index(&workspace.config().name)
    });
  }

  /// Keybinding configs that should be active for the current binding mode
  /// and pause state.
  ///
  /// When paused, only the configs with `InvokeCommand::WmTogglePause` are
  /// returned so that unpausing remains possible.
  pub fn active_keybinding_configs(
    &self,
    binding_modes: &[wm_common::BindingModeConfig],
    is_paused: bool,
  ) -> impl Iterator<Item = KeybindingConfig> {
    let source_configs = if let Some(first_mode) = binding_modes.first() {
      &first_mode.keybindings
    } else {
      &self.value.keybindings
    }
    .clone();

    source_configs.into_iter().filter(move |kb| {
      if is_paused {
        kb.commands
          .contains(&wm_common::InvokeCommand::WmTogglePause)
      } else {
        true
      }
    })
  }
}
