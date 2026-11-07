//! What each runspace that imported the module has of its own, as each import of PSWriteColorEX
//! has script variables of its own: the color names Register-ColorName added, the color support
//! the first write detected, and whether the host lets escape codes through. An import starts its
//! runspace's state afresh and the removal of that import drops it. With one runspace a read takes
//! that runspace's state and asks .NET nothing; with more, a command finds its runspace once,
//! through [`enter`]. A crate that links the library without the `module` feature has one state
//! for the process. Any thread reads the states without waiting.

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use pwrs::prelude::*;
use subetha_core::swap_cell::SwapCell;

use crate::colors::Entry;
use crate::set_once::SetOnce;

/// One runspace's state.
pub struct Session {
    /// The runspace's InstanceId: none for an import made on a thread with no default runspace,
    /// and for the state of a crate that links the library without the module.
    runspace: Option<PsGuid>,
    /// The names Register-ColorName added, in the order they were first added, each with its
    /// spelling from then. A change swaps in a new table.
    pub names: SwapCell<Vec<(String, Entry)>>,
    /// The color support and bold rendering the first write detected, as
    /// detect::session_support writes them; 0 before.
    pub support: AtomicU8,
    /// Whether the host lets escape codes through: 0 before it is read, 1 when it does not, 2
    /// when it does.
    pub virtual_terminal: AtomicU8,
}

impl Session {
    fn new(runspace: Option<PsGuid>) -> Session {
        Session { runspace, names: SwapCell::new(Vec::new()), support: AtomicU8::new(0), virtual_terminal: AtomicU8::new(0) }
    }
}

/// The states of the runspaces that imported the module, in the order of their imports.
struct Sessions {
    open: SwapCell<Vec<Arc<Session>>>,
}

impl Sessions {
    fn new() -> Sessions {
        Sessions { open: SwapCell::new(Vec::new()) }
    }

    /// Starts `runspace`'s state afresh, dropping one it had.
    fn begin(&self, runspace: Option<PsGuid>) {
        let fresh = Arc::new(Session::new(runspace));
        self.open.rcu(|open| open.iter().filter(|session| session.runspace != runspace).cloned().chain([fresh.clone()]).collect());
    }

    /// Drops `runspace`'s state.
    fn end(&self, runspace: Option<PsGuid>) {
        self.open.rcu(|open| open.iter().filter(|session| session.runspace != runspace).cloned().collect());
    }

    /// Whether more than one runspace has a state, so that a read needs the runspace it is for.
    fn shared(&self) -> bool {
        self.open.load().len() > 1
    }

    /// `runspace`'s state, or for a runspace that has none a fresh one, which nothing keeps.
    fn of(&self, runspace: Option<PsGuid>) -> Arc<Session> {
        match self.open.load().iter().find(|session| session.runspace == runspace) {
            Some(found) => found.clone(),
            None => Arc::new(Session::new(runspace)),
        }
    }

    /// Runs `f` with the state a read takes: the one runspace's when one runspace has a state,
    /// `scoped` when more do, and `library` when none does. A read with more than one and no
    /// scope, which no command of the module makes, takes a fresh state, which nothing keeps.
    fn with<R>(&self, scoped: Option<&Session>, library: &Session, f: impl FnOnce(&Session) -> R) -> R {
        let open = self.open.load();
        match (open.as_slice(), scoped) {
            ([], _) => f(library),
            ([only], _) => f(only),
            (_, Some(session)) => f(session),
            (_, None) => f(&Session::new(None)),
        }
    }
}

/// The process's states.
fn sessions() -> &'static Sessions {
    static SESSIONS: SetOnce<Sessions> = SetOnce::new();
    SESSIONS.get_or_init(Sessions::new)
}

/// The one state of a crate that links the library without the module.
fn library() -> &'static Session {
    static LIBRARY: SetOnce<Session> = SetOnce::new();
    LIBRARY.get_or_init(|| Session::new(None))
}

thread_local! {
    /// The state of the runspace the thread's command runs in, while a command that entered it
    /// runs and more than one runspace has the module.
    static SCOPED: RefCell<Option<Arc<Session>>> = const { RefCell::new(None) };
}

/// Starts the importing runspace's state afresh, for the module's import hook.
pub fn begin_import() -> PsResult<()> {
    sessions().begin(current_runspace()?);
    Ok(())
}

/// Drops the state of the runspace whose import was removed, for the module's removal hook.
pub fn end_import(runspace: Option<PsGuid>) {
    sessions().end(runspace);
}

/// A command's hold on the state of the runspace it runs in, which every read takes until the
/// scope drops.
#[must_use = "the scope lasts until it drops"]
pub struct Scope {
    entered: bool,
}

impl Drop for Scope {
    fn drop(&mut self) {
        if self.entered {
            SCOPED.with(|scoped| *scoped.borrow_mut() = None);
        }
    }
}

/// Finds the runspace the thread's command runs in, when more than one runspace has the module,
/// so that every read until the scope drops takes its state. A scope entered within another, or
/// with the module in one runspace, finds nothing and asks .NET nothing.
pub fn enter() -> PsResult<Scope> {
    if !sessions().shared() || SCOPED.with(|scoped| scoped.borrow().is_some()) {
        return Ok(Scope { entered: false });
    }
    let session = sessions().of(current_runspace()?);
    SCOPED.with(|scoped| *scoped.borrow_mut() = Some(session));
    Ok(Scope { entered: true })
}

/// Runs `f` with the state of the runspace a read is for.
pub fn with_current<R>(f: impl FnOnce(&Session) -> R) -> R {
    let sessions = sessions();
    if !sessions.shared() {
        return sessions.with(None, library(), f);
    }
    let scoped = SCOPED.with(|scoped| scoped.borrow().clone());
    sessions.with(scoped.as_deref(), library(), f)
}

/// A yes-or-no answer the runspace's state keeps in `slot`: the one kept, or else `read`'s,
/// which is kept.
pub fn kept_answer(slot: fn(&Session) -> &AtomicU8, read: impl FnOnce() -> bool) -> bool {
    let kept = with_current(|session| slot(session).load(Ordering::Acquire));
    if kept != 0 {
        return kept == 2;
    }
    let answer = read();
    with_current(|session| slot(session).store(if answer { 2 } else { 1 }, Ordering::Release));
    answer
}

/// The InstanceId of the runspace the thread runs, none on a thread with no default runspace.
fn current_runspace() -> PsResult<Option<PsGuid>> {
    let runspace = PsType::from_name("System.Management.Automation.Runspaces.Runspace").call_static("get_DefaultRunspace", &[])?;
    if runspace.is_null() {
        return Ok(None);
    }
    Ok(Some(PsGuid::from_ps(&runspace.get("InstanceId")?)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runspace(n: u8) -> Option<PsGuid> {
        Some(PsGuid { bytes: [n; 16] })
    }

    fn add_name(session: &Session) {
        session.names.rcu(|names| names.iter().cloned().chain([("ZzSessionTestName".to_string(), Entry::from_rgb([1, 2, 3]))]).collect());
    }

    fn name_count(sessions: &Sessions, scoped: Option<&Session>, library: &Session) -> usize {
        sessions.with(scoped, library, |session| session.names.load().len())
    }

    #[test]
    fn with_no_import_every_read_takes_the_one_state_of_the_process() {
        let sessions = Sessions::new();
        let library = Session::new(None);
        sessions.with(None, &library, add_name);
        assert_eq!(name_count(&sessions, None, &library), 1);
        assert_eq!(library.names.load().len(), 1);
    }

    #[test]
    fn each_runspace_reads_its_own_names_and_an_import_starts_them_afresh() {
        let sessions = Sessions::new();
        let library = Session::new(None);
        sessions.begin(runspace(1));
        assert!(!sessions.shared());
        sessions.with(None, &library, add_name);
        sessions.begin(runspace(2));
        assert!(sessions.shared());
        let first = sessions.of(runspace(1));
        let second = sessions.of(runspace(2));
        assert_eq!(name_count(&sessions, Some(&*first), &library), 1);
        assert_eq!(name_count(&sessions, Some(&*second), &library), 0);
        sessions.with(Some(&*second), &library, add_name);
        assert_eq!(sessions.of(runspace(2)).names.load().len(), 1);
        sessions.begin(runspace(2));
        assert_eq!(sessions.of(runspace(2)).names.load().len(), 0, "importing again starts the runspace's names afresh");
        assert_eq!(sessions.of(runspace(1)).names.load().len(), 1);
        assert_eq!(library.names.load().len(), 0);
    }

    #[test]
    fn a_removal_drops_its_runspace_and_the_one_left_is_read_without_a_scope() {
        let sessions = Sessions::new();
        let library = Session::new(None);
        sessions.begin(runspace(1));
        sessions.begin(runspace(2));
        let first = sessions.of(runspace(1));
        sessions.with(Some(&*first), &library, add_name);
        sessions.end(runspace(2));
        assert!(!sessions.shared());
        assert_eq!(name_count(&sessions, None, &library), 1);
        sessions.end(runspace(1));
        assert_eq!(name_count(&sessions, None, &library), 0);
    }

    #[test]
    fn a_runspace_with_no_state_reads_a_fresh_one() {
        let sessions = Sessions::new();
        sessions.begin(runspace(1));
        sessions.begin(runspace(2));
        assert_eq!(sessions.of(runspace(3)).names.load().len(), 0);
        assert_eq!(sessions.open.load().len(), 2, "a runspace with no import adds no state");
    }
}
