use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize};

use crate::app_state::{
    AppState, LeafContents, PaneNodeSnapshot, SplitDirection as StateSplitDirection, TabSnapshot,
    WindowSnapshot,
};
use crate::themes::theme::AnsiColorIdentifier;

#[cfg(test)]
#[path = "launch_config_tests.rs"]
mod tests;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct LaunchConfig {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub active_window_index: Option<usize>,
    pub windows: Vec<WindowTemplate>,
}

impl LaunchConfig {
    pub fn from_snapshot(name: String, app_state: &AppState) -> Self {
        Self {
            name,
            active_window_index: app_state.active_window_index,
            windows: app_state
                .windows
                .iter()
                .filter_map(|window| (!window.quake_mode).then_some(window.clone().into()))
                .collect::<Vec<WindowTemplate>>(),
        }
    }

    /// Rewrites every pane's working directory to `cwd`, recursing into split branches. Used to
    /// re-root a path-less template at an arbitrary directory when opening it as a project (the
    /// `newds` command and "open template here" both rely on this).
    pub fn rewrite_cwds(&mut self, cwd: &Path) {
        fn rewrite(layout: &mut PaneTemplateType, cwd: &Path) {
            match layout {
                PaneTemplateType::PaneTemplate { cwd: pane_cwd, .. } => {
                    *pane_cwd = Some(cwd.to_path_buf());
                }
                PaneTemplateType::PaneBranchTemplate { panes, .. } => {
                    for pane in panes {
                        rewrite(pane, cwd);
                    }
                }
            }
        }
        for window in &mut self.windows {
            for tab in &mut window.tabs {
                rewrite(&mut tab.layout, cwd);
            }
        }
    }

    /// Builds a minimal single-window, single-tab, single-pane config opening a plain shell at
    /// `cwd`. Used as the `newds` fallback when no "default" template config exists.
    pub fn single_pane(name: String, cwd: PathBuf) -> Self {
        Self {
            name,
            active_window_index: Some(0),
            windows: vec![WindowTemplate {
                active_tab_index: Some(0),
                tabs: vec![TabTemplate {
                    title: None,
                    layout: PaneTemplateType::PaneTemplate {
                        cwd: Some(cwd),
                        commands: Vec::new(),
                        is_focused: Some(true),
                        pane_mode: PaneMode::Terminal,
                        shell: None,
                    },
                    color: None,
                }],
            }],
        }
    }

    /// Returns the working directory of the first pane in the first tab of the first window, if any.
    /// Used by the `projects:` palette to show a project's path and resolve its current git branch.
    /// Returns `None` for a path-less template (no pane carries a `cwd`).
    pub fn primary_cwd(&self) -> Option<&Path> {
        fn first_cwd(layout: &PaneTemplateType) -> Option<&Path> {
            match layout {
                PaneTemplateType::PaneTemplate { cwd, .. } => cwd.as_deref(),
                PaneTemplateType::PaneBranchTemplate { panes, .. } => {
                    panes.iter().find_map(first_cwd)
                }
            }
        }
        self.windows
            .first()?
            .tabs
            .first()
            .and_then(|tab| first_cwd(&tab.layout))
    }

    /// Whether this config is a path-less *template*: no pane in any tab/window carries a `cwd`.
    /// A template defines layout + commands only and is opened *at* a path supplied at launch time;
    /// a config with at least one baked-in `cwd` is a concrete *project*.
    pub fn is_template(&self) -> bool {
        fn has_cwd(layout: &PaneTemplateType) -> bool {
            match layout {
                PaneTemplateType::PaneTemplate { cwd, .. } => cwd.is_some(),
                PaneTemplateType::PaneBranchTemplate { panes, .. } => panes.iter().any(has_cwd),
            }
        }
        !self
            .windows
            .iter()
            .flat_map(|window| window.tabs.iter())
            .any(|tab| has_cwd(&tab.layout))
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct WindowTemplate {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub active_tab_index: Option<usize>,
    pub tabs: Vec<TabTemplate>,
}

impl From<WindowSnapshot> for WindowTemplate {
    fn from(snapshot: WindowSnapshot) -> Self {
        let mut active_tab_index = None;
        let mut num_valid_tabs = 0;

        let tabs = snapshot
            .tabs
            .into_iter()
            .enumerate()
            .filter_map(|(i, tab)| {
                let tab = tab.try_into().ok()?;

                if i == snapshot.active_tab_index {
                    active_tab_index = Some(num_valid_tabs);
                }

                num_valid_tabs += 1;

                Some(tab)
            })
            .collect::<Vec<TabTemplate>>();

        Self {
            active_tab_index,
            tabs,
        }
    }
}

fn is_falsey(val: &Option<bool>) -> bool {
    val.is_none_or(|v| !v)
}

/// The mode a leaf pane opens in.
///
/// Used by tab configs to distinguish terminal, agent, and cloud panes.
/// Launch configs always produce `Terminal` (the default).
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaneMode {
    /// A standard terminal shell session.
    #[default]
    Terminal,
    /// A terminal that immediately enters Agent Mode.
    Agent,
    /// A cloud-mode (ambient agent) pane with no local shell.
    Cloud,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(untagged, rename_all = "lowercase")]
pub enum PaneTemplateType {
    // NOTE: `PaneBranchTemplate` must come first. With `untagged`, serde tries variants top to
    // bottom; since every `PaneTemplate` field is optional, `PaneTemplate` would otherwise match a
    // split node `{split_direction, panes}` and silently drop the split. `PaneBranchTemplate` has
    // required `split_direction` + `panes`, so leaf panes fail it and correctly fall through to
    // `PaneTemplate`.
    PaneBranchTemplate {
        split_direction: SplitDirection,
        panes: Vec<PaneTemplateType>,
    },
    PaneTemplate {
        /// Working directory for this pane. `None` marks a path-less *template* pane, opened at a
        /// path supplied at launch time (see [`LaunchConfig::is_template`]).
        #[serde(
            deserialize_with = "deserialize_optional_path",
            skip_serializing_if = "Option::is_none",
            default
        )]
        cwd: Option<PathBuf>,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        commands: Vec<CommandTemplate>,
        #[serde(skip_serializing_if = "is_falsey", default)]
        is_focused: Option<bool>,
        #[serde(default)]
        pane_mode: PaneMode,
        /// Optional shell override for this pane (e.g. `"pwsh"`, `"zsh"`).
        /// Sourced from the `shell` field of a tab config pane node.
        #[serde(skip_serializing_if = "Option::is_none", default)]
        shell: Option<String>,
    },
}

impl TryFrom<PaneNodeSnapshot> for PaneTemplateType {
    type Error = ();

    #[allow(clippy::unwrap_in_result)]
    fn try_from(snapshot: PaneNodeSnapshot) -> Result<Self, ()> {
        match snapshot {
            PaneNodeSnapshot::Branch(branch) => {
                let panes = branch
                    .children
                    .iter()
                    .filter_map(|(_, snapshot)| snapshot.clone().try_into().ok())
                    .collect::<Vec<PaneTemplateType>>();
                match panes.len() {
                    0 => Err(()),
                    1 => Ok(panes
                        .into_iter()
                        .next()
                        .expect("Checked that panes has 1 element")),
                    _ => Ok(Self::PaneBranchTemplate {
                        split_direction: branch.direction.into(),
                        panes,
                    }),
                }
            }
            PaneNodeSnapshot::Leaf(leaf) => match leaf.contents {
                LeafContents::Terminal(terminal) => Ok(Self::PaneTemplate {
                    cwd: terminal.cwd.map(PathBuf::from),
                    commands: Vec::new(),
                    is_focused: Some(leaf.is_focused),
                    pane_mode: PaneMode::Terminal,
                    shell: None,
                }),
                // Currently, notebook panes cannot be saved in launch configurations.
                LeafContents::Notebook(_)
                | LeafContents::EnvVarCollection(_)
                | LeafContents::Code(_)
                | LeafContents::Workflow(_)
                | LeafContents::Settings(_)
                | LeafContents::AIFact(_)
                | LeafContents::CodeReview(_)
                | LeafContents::ExecutionProfileEditor
                | LeafContents::GetStarted
                | LeafContents::NetworkLog
                | LeafContents::Welcome { .. }
                | LeafContents::AIDocument(_)
                | LeafContents::EnvironmentManagement(_)
                | LeafContents::AmbientAgent(_) => {
                    // TODO: Handle AIDocument in launch config
                    Err(())
                }
            },
        }
    }
}

/// Deserializes an optional string that semantically represents a path, expanding ~ as needed.
/// A missing `cwd` key (template pane) deserializes to `None`.
fn deserialize_optional_path<'de, D>(deserializer: D) -> Result<Option<PathBuf>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw_path = Option::<String>::deserialize(deserializer)?;
    Ok(raw_path.map(|p| PathBuf::from(shellexpand::tilde(&p).into_owned())))
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct TabTemplate {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub title: Option<String>,
    pub layout: PaneTemplateType,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub color: Option<AnsiColorIdentifier>,
}

impl TryFrom<TabSnapshot> for TabTemplate {
    type Error = ();

    fn try_from(snapshot: TabSnapshot) -> Result<Self, ()> {
        let color = snapshot.color();
        Ok(Self {
            title: snapshot.custom_title,
            layout: snapshot.root.try_into()?,
            color,
        })
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SplitDirection {
    Vertical,
    Horizontal,
}

impl From<StateSplitDirection> for SplitDirection {
    fn from(snapshot: StateSplitDirection) -> Self {
        match snapshot {
            StateSplitDirection::Horizontal => Self::Horizontal,
            StateSplitDirection::Vertical => Self::Vertical,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct CommandTemplate {
    pub exec: String,
}

impl From<&str> for CommandTemplate {
    fn from(s: &str) -> CommandTemplate {
        CommandTemplate {
            exec: s.to_string(),
        }
    }
}

// TODO add extra elements to the mock (split panes, multiple tabs, multiple windows)
pub fn make_mock_single_window_launch_config() -> LaunchConfig {
    LaunchConfig {
        name: "Mocked Config".to_string(),
        active_window_index: Some(0),
        windows: vec![WindowTemplate {
            active_tab_index: Some(0),
            tabs: vec![
                TabTemplate {
                    title: Some("First Tab".to_string()),
                    layout: PaneTemplateType::PaneTemplate {
                        is_focused: Some(true),
                        cwd: Some(PathBuf::from("/some/path")),
                        commands: vec!["echo test_command".into()],
                        pane_mode: PaneMode::Terminal,
                        shell: None,
                    },
                    color: None,
                },
                TabTemplate {
                    title: Some("Second Tab".to_string()),
                    layout: PaneTemplateType::PaneTemplate {
                        is_focused: Some(true),
                        cwd: Some(PathBuf::from("/some/path")),
                        commands: vec!["echo test_command_on_another_tab".into()],
                        pane_mode: PaneMode::Terminal,
                        shell: None,
                    },
                    color: None,
                },
            ],
        }],
    }
}
