use crate::log_window::open_log_window;
use crate::state::{ActivityKind, Globals, KeyTarget, Member, SessionInfo, Status, View};
use crate::widgets::{
    ACCENT, DANGER, MUTED, SUCCESS, WARNING, dot, error_banner, field, scroll_section, section,
};
use crate::{connection, settings};
use flixparty_core::config::default_playback_key;
use flixparty_core::periphery::key_name;
use freya::prelude::*;
use rdev::Key;
use tracing::error;

pub struct App {
    pub globals: Globals,
}

impl freya::prelude::App for App {
    fn render(&self) -> impl IntoElement {
        use_init_theme(dark_theme);
        let g = self.globals;

        let body = match *g.view.read() {
            View::Settings => settings_view(g),
            View::Session => session_view(g),
        };

        rect()
            .expanded()
            .theme_background()
            .theme_color()
            .content(Content::Flex)
            .child(header(g))
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::flex(1.))
                    .child(body),
            )
    }
}

fn header(mut g: Globals) -> Element {
    let action = match *g.view.read() {
        View::Settings => Button::new()
            .compact()
            .filled()
            .on_press(move |_| on_connect(g))
            .child("Connect"),
        View::Session => Button::new()
            .compact()
            .on_press(move |_| {
                connection::disconnect();
                g.session.set(None);
                g.view.set(View::Settings);
            })
            .child("Disconnect"),
    };

    rect()
        .width(Size::fill())
        .horizontal()
        .content(Content::Flex)
        .cross_align(Alignment::center())
        .spacing(8.)
        .padding((10., 16.))
        .background((30, 30, 30))
        .child(
            label()
                .width(Size::flex(1.))
                .text("flixparty")
                .font_size(20.)
                .font_weight(FontWeight::BOLD),
        )
        .child(action)
        .child(
            Button::new()
                .compact()
                .on_press(move |_| open_log_window(g))
                .child("Show Logs"),
        )
        .into()
}

// --- Settings -------------------------------------------------------------

fn settings_view(g: Globals) -> Element {
    let form = g.form;

    let keys = section(
        "Key Configuration",
        vec![
            key_field(g, "Toggle Key", KeyTarget::Toggle),
            key_field(g, "Playback Key", KeyTarget::Playback),
        ],
    );

    let connection = section(
        "Connection Information",
        vec![
            field(
                "Redis Address",
                None,
                Input::new(form.address)
                    .placeholder("redis.example.com:6379")
                    .width(Size::fill()),
            ),
            rect()
                .horizontal()
                .spacing(10.)
                .cross_align(Alignment::center())
                .child(Switch::new().toggled(form.tls).on_toggle(move |_| {
                    let mut tls = form.tls;
                    tls.toggle();
                }))
                .child("Use TLS")
                .into(),
            field(
                "Redis Username",
                Some("optional"),
                Input::new(form.username).width(Size::fill()),
            ),
            field(
                "Redis Password",
                Some("optional"),
                Input::new(form.password)
                    .mode(InputMode::new_password())
                    .width(Size::fill()),
            ),
            field(
                "Redis Channel",
                Some("optional"),
                Input::new(form.channel)
                    .placeholder("flixparty")
                    .width(Size::fill()),
            ),
            field(
                "Display Name",
                Some("optional"),
                Input::new(form.name)
                    .placeholder(g.config.read().connection.display_name())
                    .width(Size::fill()),
            ),
        ],
    );

    let mut conditions = Vec::new();
    if cfg!(target_os = "windows") {
        conditions.push(field(
            "Match Window Class",
            Some("optional"),
            Input::new(form.class)
                .placeholder("Chrome_WidgetWin_1")
                .width(Size::fill()),
        ));
    }
    conditions.push(field(
        "Window Title Contains",
        Some("optional"),
        Input::new(form.title_contains)
            .placeholder("Netflix")
            .width(Size::fill()),
    ));
    conditions.push(
        label()
            .text(
                "The toggle key is only sent while a browser or a window matching \
                 these conditions is focused.",
            )
            .font_size(12.)
            .color(MUTED)
            .into(),
    );
    let conditions = section("Conditions", conditions);

    let error = g.error.read().clone();

    ScrollView::new()
        .expanded()
        .child(
            rect()
                .width(Size::fill())
                .padding(16.)
                .spacing(14.)
                .child(keys)
                .child(connection)
                .child(conditions)
                .maybe_child(error.map(error_banner)),
        )
        .into()
}

fn key_field(g: Globals, name: &str, target: KeyTarget) -> Element {
    let form = g.form;
    let mut recording = form.recording;
    let mut key = match target {
        KeyTarget::Toggle => form.toggle_key,
        KeyTarget::Playback => form.playback_key,
    };
    let default = match target {
        KeyTarget::Toggle => Key::KeyP,
        KeyTarget::Playback => default_playback_key(),
    };
    let is_recording = *recording.read() == Some(target);

    let key_label = if is_recording {
        label().text("Press a key ...").color(ACCENT)
    } else {
        label().text(key_name(*key.read()))
    };

    let row = rect()
        .width(Size::fill())
        .horizontal()
        .content(Content::Flex)
        .spacing(8.)
        .cross_align(Alignment::center())
        .child(
            rect()
                .width(Size::flex(1.))
                .padding((8., 10.))
                .corner_radius(6.)
                .background((45, 45, 45))
                .child(key_label.font_weight(FontWeight::BOLD)),
        )
        .child(
            Button::new()
                .compact()
                .on_press(move |_| {
                    if is_recording {
                        recording.set(None);
                    } else {
                        recording.set(Some(target));
                    }
                })
                .child(if is_recording { "Cancel" } else { "Record" }),
        )
        .child(
            Button::new()
                .compact()
                .flat()
                .enabled(*key.read() != default)
                .on_press(move |_| key.set(default))
                .child("Reset"),
        );

    field(name, None, row)
}

fn on_connect(mut g: Globals) {
    g.form.recording.set(None);

    let cfg = g.form.to_config(&g.config.read());
    if let Err(err) = cfg.validate() {
        g.error.set(Some(err.to_string()));
        return;
    }

    if let Err(err) = settings::save(&cfg) {
        error!("Failed saving settings: {err}");
    }

    let channel = cfg.connection.channel.clone();
    g.config.set(cfg.clone());

    match connection::connect(cfg) {
        Ok(started) => {
            g.error.set(None);
            g.session.set(Some(SessionInfo::new(
                started.generation,
                channel,
                started.client_id,
                started.name,
            )));
            g.view.set(View::Session);
        }
        Err(err) => g.error.set(Some(err.to_string())),
    }
}

// --- Session --------------------------------------------------------------

fn session_view(g: Globals) -> Element {
    let session = g.session.read();
    let Some(session) = session.as_ref() else {
        return rect().into();
    };

    let (color, status) = match &session.status {
        Status::Connecting => (WARNING, "Connecting ...".to_string()),
        Status::Connected => (SUCCESS, "Connected".to_string()),
        Status::Reconnecting { error, remaining } => (
            WARNING,
            format!("Reconnecting ({remaining} retries left): {error}"),
        ),
        Status::Disconnected => (DANGER, "Disconnected".to_string()),
    };

    let status = section(
        "Connection",
        vec![
            rect()
                .horizontal()
                .spacing(8.)
                .cross_align(Alignment::center())
                .child(dot(color))
                .child(label().text(status))
                .into(),
            label()
                .text(format!(
                    "Channel {} on {}",
                    session.channel,
                    g.config.read().connection.address
                ))
                .font_size(13.)
                .color(MUTED)
                .into(),
        ],
    );

    let members = scroll_section(
        &format!("Members ({})", session.members.len()),
        session
            .members
            .iter()
            .map(|m| member_row(m, session.last_toggle_by.as_deref() == Some(&m.id)))
            .collect(),
    );

    let activity: Vec<Element> = if session.activity.is_empty() {
        vec![label().text("Nothing happened yet.").color(MUTED).into()]
    } else {
        session
            .activity
            .iter()
            .map(|a| {
                let color = match a.kind {
                    ActivityKind::Toggle => ACCENT,
                    ActivityKind::Info => (210, 210, 210),
                    ActivityKind::Warning => WARNING,
                };
                rect()
                    .width(Size::fill())
                    .horizontal()
                    .spacing(10.)
                    .child(label().text(a.time.clone()).font_size(13.).color(MUTED))
                    .child(label().text(a.text.clone()).font_size(13.).color(color))
                    .into()
            })
            .collect()
    };
    let activity = scroll_section("Activity", activity);

    // Members and activity share the height left over by the status section.
    rect()
        .expanded()
        .padding(16.)
        .spacing(14.)
        .content(Content::Flex)
        .child(status)
        .child(members)
        .child(activity)
        .into()
}

fn member_row(member: &Member, last_toggled: bool) -> Element {
    let mut name = member.name.clone();
    if member.is_self {
        name.push_str(" (you)");
    }

    let toggle = member.last_toggle.as_ref().map(|t| {
        label()
            .text(format!("toggled at {t}"))
            .font_size(12.)
            .color(if last_toggled { ACCENT } else { MUTED })
    });

    rect()
        .width(Size::fill())
        .horizontal()
        .content(Content::Flex)
        .spacing(10.)
        .padding((8., 10.))
        .corner_radius(6.)
        .cross_align(Alignment::center())
        .background(if last_toggled { (35, 50, 70) } else { (40, 40, 40) })
        .child(dot(if member.is_self { ACCENT } else { SUCCESS }))
        .child(
            rect()
                .width(Size::flex(1.))
                .spacing(2.)
                .child(label().text(name).font_weight(FontWeight::BOLD))
                .child(label().text(member.id.clone()).font_size(11.).color(MUTED)),
        )
        .maybe_child(toggle)
        .into()
}
