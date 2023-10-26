use crate::Project;
use gpui2::{AnyWindowHandle, Context, Handle, ModelContext, WeakHandle};
use settings2::Settings;
use std::path::{Path, PathBuf};
use terminal2::{
    terminal_settings::{self, TerminalSettings, VenvSettingsContent},
    Terminal, TerminalBuilder,
};

#[cfg(target_os = "macos")]
use std::os::unix::ffi::OsStrExt;

pub struct Terminals {
    pub(crate) local_handles: Vec<WeakHandle<terminal2::Terminal>>,
}

impl Project {
    pub fn create_terminal(
        &mut self,
        working_directory: Option<PathBuf>,
        window: AnyWindowHandle,
        cx: &mut ModelContext<Self>,
    ) -> anyhow::Result<Handle<Terminal>> {
        if self.is_remote() {
            return Err(anyhow::anyhow!(
                "creating terminals as a guest is not supported yet"
            ));
        } else {
            let settings = TerminalSettings::get_global(cx);
            let python_settings = settings.detect_venv.clone();
            let shell = settings.shell.clone();

            let terminal = TerminalBuilder::new(
                working_directory.clone(),
                shell.clone(),
                settings.env.clone(),
                Some(settings.blinking.clone()),
                settings.alternate_scroll,
                window,
                |_, _| todo!("color_for_index"),
            )
            .map(|builder| {
                let terminal_handle = cx.entity(|cx| builder.subscribe(cx));

                self.terminals
                    .local_handles
                    .push(terminal_handle.downgrade());

                let id = terminal_handle.entity_id();
                cx.observe_release(&terminal_handle, move |project, _terminal, cx| {
                    let handles = &mut project.terminals.local_handles;

                    if let Some(index) = handles
                        .iter()
                        .position(|terminal| terminal.entity_id() == id)
                    {
                        handles.remove(index);
                        cx.notify();
                    }
                })
                .detach();

                if let Some(python_settings) = &python_settings.as_option() {
                    let activate_script_path =
                        self.find_activate_script_path(&python_settings, working_directory);
                    self.activate_python_virtual_environment(
                        activate_script_path,
                        &terminal_handle,
                        cx,
                    );
                }
                terminal_handle
            });

            terminal
        }
    }

    pub fn find_activate_script_path(
        &mut self,
        settings: &VenvSettingsContent,
        working_directory: Option<PathBuf>,
    ) -> Option<PathBuf> {
        // When we are unable to resolve the working directory, the terminal builder
        // defaults to '/'. We should probably encode this directly somewhere, but for
        // now, let's just hard code it here.
        let working_directory = working_directory.unwrap_or_else(|| Path::new("/").to_path_buf());
        let activate_script_name = match settings.activate_script {
            terminal_settings::ActivateScript::Default => "activate",
            terminal_settings::ActivateScript::Csh => "activate.csh",
            terminal_settings::ActivateScript::Fish => "activate.fish",
            terminal_settings::ActivateScript::Nushell => "activate.nu",
        };

        for virtual_environment_name in settings.directories {
            let mut path = working_directory.join(virtual_environment_name);
            path.push("bin/");
            path.push(activate_script_name);

            if path.exists() {
                return Some(path);
            }
        }

        None
    }

    fn activate_python_virtual_environment(
        &mut self,
        activate_script: Option<PathBuf>,
        terminal_handle: &Handle<Terminal>,
        cx: &mut ModelContext<Project>,
    ) {
        if let Some(activate_script) = activate_script {
            // Paths are not strings so we need to jump through some hoops to format the command without `format!`
            let mut command = Vec::from("source ".as_bytes());
            command.extend_from_slice(activate_script.as_os_str().as_bytes());
            command.push(b'\n');

            terminal_handle.update(cx, |this, _| this.input_bytes(command));
        }
    }

    pub fn local_terminal_handles(&self) -> &Vec<WeakHandle<terminal2::Terminal>> {
        &self.terminals.local_handles
    }
}

// TODO: Add a few tests for adding and removing terminal tabs
