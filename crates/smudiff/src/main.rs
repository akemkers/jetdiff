//! smudiff <venstre> <høyre> [--light]

use std::process::ExitCode;

use gpui::{
    App, AppContext, Application, Bounds, Focusable, KeyBinding, WindowBounds, WindowOptions,
    actions, px, size,
};
use smudiff_ui::text::load_font;
use smudiff_ui::{
    CollapseAllFolds, CollapseFold, ExpandAllFolds, ExpandFold, FileDiffView, NextChange,
    NextContextRange, PreviousChange, UiTheme,
};
use smudiff_view::{ComparisonPolicy, FileDiff};

actions!(smudiff, [Quit]);

struct Args {
    left: String,
    right: String,
    light: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut light = false;
    let mut paths = Vec::new();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--light" => light = true,
            "--dark" => light = false,
            s if s.starts_with("--") => return Err(format!("ukjent flagg: {s}")),
            _ => paths.push(arg),
        }
    }
    match <[String; 2]>::try_from(paths) {
        Ok([left, right]) => Ok(Args { left, right, light }),
        Err(_) => Err("bruk: smudiff <venstre> <høyre> [--light]".into()),
    }
}

fn read(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    // Binærfiler og feil encoding vises med erstatningstegn i stedet for å feile
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("f7", NextChange, None),
        KeyBinding::new("shift-f7", PreviousChange, None),
        // IntelliJ macOS-keymap. GPUI gjør cmd-shift-= til cmd-+ og cmd-shift-- til cmd-_
        KeyBinding::new("cmd-=", ExpandFold, None),
        KeyBinding::new("cmd--", CollapseFold, None),
        KeyBinding::new("cmd-+", ExpandAllFolds, None),
        KeyBinding::new("cmd-_", CollapseAllFolds, None),
        // IntelliJ har ingen snarvei, bare en meny i verktøylinja
        KeyBinding::new("cmd-alt-c", NextContextRange, None),
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-w", Quit, None),
    ]);
    cx.on_action(|_: &Quit, cx| cx.quit());
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let (left, right) = match (read(&args.left), read(&args.right)) {
        (Ok(l), Ok(r)) => (l, r),
        (Err(e), _) | (_, Err(e)) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    let diff = match FileDiff::new(&left, &right, ComparisonPolicy::Default) {
        Ok(d) => d,
        Err(_) => {
            eprintln!("filene er for store til å diffes");
            return ExitCode::from(1);
        }
    };
    let theme = if args.light {
        UiTheme::light()
    } else {
        UiTheme::dark()
    };
    let title = format!("{} ↔ {}", args.left, args.right);

    Application::new().run(move |cx: &mut App| {
        bind_keys(cx);
        let family = load_font(cx);
        let bounds = Bounds::centered(None, size(px(1400.), px(900.)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(gpui::TitlebarOptions {
                        title: Some(title.into()),
                        ..Default::default()
                    }),
                    focus: true,
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| FileDiffView::new(diff, theme, family, cx));
                    window.focus(&view.focus_handle(cx));
                    view
                },
            )
            .expect("kunne ikke åpne vindu");
        let _ = window;
        cx.on_window_closed(|cx| cx.quit()).detach();
        cx.activate(true);
    });
    ExitCode::SUCCESS
}
