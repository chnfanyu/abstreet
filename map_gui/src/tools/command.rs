use std::collections::VecDeque;
use std::time::Duration;

use instant::Instant;
use subprocess::{Communicator, Popen};

use widgetry::{Color, EventCtx, GfxCtx, Line, Panel, State, Text, Transition, UpdateType};

use crate::tools::PopupMsg;
use crate::AppLike;

/// Executes a command and displays STDOUT and STDERR in a loading screen window. Only works on
/// native, of course.
pub struct RunCommand {
    p: Popen,
    comm: Communicator,
    panel: Panel,
    lines: VecDeque<String>,
    max_capacity: usize,
    started: Instant,
}

impl RunCommand {
    pub fn new<A: AppLike + 'static>(
        ctx: &mut EventCtx,
        _: &A,
        args: Vec<&str>,
    ) -> Box<dyn State<A>> {
        match subprocess::Popen::create(
            &args,
            subprocess::PopenConfig {
                stdout: subprocess::Redirection::Pipe,
                stderr: subprocess::Redirection::Pipe,
                ..Default::default()
            },
        ) {
            Ok(mut p) => {
                let comm = p
                    .communicate_start(None)
                    .limit_time(Duration::from_millis(0));
                let panel = ctx.make_loading_screen(Text::from(Line("Starting command...")));
                let max_capacity =
                    (0.8 * ctx.canvas.window_height / ctx.default_line_height()) as usize;
                Box::new(RunCommand {
                    p,
                    comm,
                    panel,
                    lines: VecDeque::new(),
                    max_capacity,
                    started: Instant::now(),
                })
            }
            Err(err) => PopupMsg::new(
                ctx,
                "Error",
                vec![format!("Couldn't start command: {}", err)],
            ),
        }
    }
}

impl<A: AppLike + 'static> State<A> for RunCommand {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut A) -> Transition<A> {
        ctx.request_update(UpdateType::Game);
        if ctx.input.nonblocking_is_update_event().is_none() {
            return Transition::Keep;
        }

        let mut new_lines = Vec::new();
        let (stdout, stderr) = match self.comm.read() {
            Ok(pair) => pair,
            // This is almost always a timeout.
            Err(err) => err.capture,
        };
        for raw in vec![stdout, stderr] {
            if let Some(bytes) = raw {
                if let Ok(string) = String::from_utf8(bytes) {
                    if !string.is_empty() {
                        for line in string.split("\n") {
                            new_lines.push(line.to_string());
                        }
                    }
                }
            }
        }
        if !new_lines.is_empty() {
            for line in new_lines {
                if self.lines.len() == self.max_capacity {
                    self.lines.pop_front();
                }
                self.lines.push_back(line);
            }
        }

        // Throttle rerendering?
        let mut txt = Text::from(
            Line(format!(
                "Running command... {} so far",
                geom::Duration::realtime_elapsed(self.started)
            ))
            .small_heading(),
        );
        for line in &self.lines {
            txt.add(Line(line));
        }
        self.panel = ctx.make_loading_screen(txt);

        if let Some(status) = self.p.poll() {
            if status.success() {
                return Transition::Replace(PopupMsg::new(
                    ctx,
                    "Success!",
                    self.lines.drain(..).collect(),
                ));
            }
            return Transition::Replace(PopupMsg::new(
                ctx,
                "Failure!",
                self.lines.drain(..).collect(),
            ));
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &A) {
        g.clear(Color::BLACK);
        self.panel.draw(g);
    }
}
