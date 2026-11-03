//! The tab's words, in English and French. English fills in what French lacks;
//! `key_one` and `key_other` are picked by a count. `strings.rs` is the page's
//! `STRINGS`, key for key (the tests hold the two languages to the same keys).
//!
//! What the agent receives is not here: it is English whatever the person
//! reads (`logic::build_summary`).

use hangar_kit_sdk::i18n::Strings;

pub const EN: &[(&str, &str)] = &[
    ("tab", "Checklist"),
    ("loading", "Reading the list…"),
    ("retry", "Try again"),
    ("untitled", "Checklist"),
    ("steps", "Steps"),
    ("ok", "OK"),
    ("problem", "Problem"),
    ("skip", "Skip"),
    ("problems", "Problems"),
    ("skipped", "Skipped"),
    ("expectedLine", "Expected: {text}"),
    ("todo", "To do"),
    ("noteLabel", "What went wrong"),
    ("notePlaceholder", "Describe what you saw instead"),
    ("redo", "To redo"),
    ("redoWas", "It failed last time: {note}"),
    ("redoWasBare", "It failed last time. Test it again."),
    ("stateRunning", "{done} of {total} answered"),
    ("stateFailure_one", "{n} step failed"),
    ("stateFailure_other", "{n} steps failed"),
    ("stateSuccess", "Everything passed"),
    ("stateSuccessSkipped", "Nothing failed"),
    ("badgeFailure", "{n} failed"),
    ("send", "Send to agent"),
    ("sendAgain", "Send again"),
    ("sending", "Sending…"),
    (
        "sentTo",
        "Sent to {pane}. It fixes what failed: open this list again to test it.",
    ),
    ("needAnswer", "Answer at least one step to send the result."),
    (
        "pending_one",
        "{n} step is still unanswered: the agent will be told.",
    ),
    (
        "pending_other",
        "{n} steps are still unanswered: the agent will be told.",
    ),
    (
        "noCaller",
        "No agent asked for this list. Copy the summary to give it to one.",
    ),
    ("copy", "Copy the summary"),
    ("copied", "Copied"),
    ("notifySent", "Result sent to {pane}"),
    (
        "persistOff",
        "Answers cannot be saved here: closing this tab loses them.",
    ),
    (
        "send.denied",
        "This tab may not send to agents. Allow panes.send for the kit in Settings › Kits.",
    ),
    (
        "send.not_found",
        "The agent that asked is no longer in this workspace. Copy the summary instead.",
    ),
    (
        "send.unavailable",
        "That agent's session cannot receive messages right now. Try again in a moment.",
    ),
    (
        "send.rate_limited",
        "Too many messages sent. Wait a minute, then send again.",
    ),
    ("send.error", "The result could not be sent: {detail}"),
    ("err.missing.title", "No list to show"),
    (
        "err.missing.text",
        "Open this tab with the path of a JSON file, for example recette/login.json.",
    ),
    ("err.path.title", "This path cannot be used"),
    (
        "err.path.text",
        "It must be relative to the workspace folder, with no .., no drive, no colon and no hidden folder.",
    ),
    ("err.not_found.title", "List not found"),
    (
        "err.not_found.text",
        "{path} does not exist in the workspace folder. The agent writes the file before it opens this tab.",
    ),
    ("err.denied.title", "This tab may not read files"),
    (
        "err.denied.text",
        "Allow files.read for the kit in Settings › Kits, then try again.",
    ),
    ("err.json.title", "The list is not valid JSON"),
    ("err.json.text", "{detail}"),
    ("err.empty.title", "The list is empty"),
    (
        "err.empty.text",
        "{path} holds nothing. The agent has to write the list in it.",
    ),
    ("err.shape.title", "The list is not in the expected shape"),
    ("err.shape.text", "{detail}"),
    ("err.error.title", "The list could not be read"),
    ("err.error.text", "{detail}"),
    (
        "shape.notObject",
        "The file must hold an object with a title and a list of steps.",
    ),
    (
        "shape.noSteps",
        "steps must be a list with at least one step.",
    ),
    ("shape.tooMany", "At most {max} steps; this list has {n}."),
    (
        "shape.stepNotObject",
        "Step {n} must be an object with a text.",
    ),
    ("shape.stepText", "Step {n} has no text."),
    ("shape.stepExpect", "Step {n}: expect must be a text."),
    (
        "shape.stepLong",
        "Step {n}: the text is longer than {max} characters.",
    ),
    ("shape.titleType", "title must be a text."),
];

pub const FR: &[(&str, &str)] = &[
    ("tab", "Recette"),
    ("loading", "Lecture de la liste…"),
    ("retry", "Réessayer"),
    ("untitled", "Liste de vérifications"),
    ("steps", "Étapes"),
    ("ok", "OK"),
    ("problem", "Problème"),
    ("skip", "Passer"),
    ("problems", "Problèmes"),
    ("skipped", "Passées"),
    ("expectedLine", "Attendu : {text}"),
    ("todo", "À faire"),
    ("noteLabel", "Ce qui ne va pas"),
    ("notePlaceholder", "Décrivez ce que vous avez vu à la place"),
    ("redo", "À refaire"),
    ("redoWas", "Échec la dernière fois : {note}"),
    ("redoWasBare", "Échec la dernière fois. Testez à nouveau."),
    ("stateRunning", "{done} sur {total} traitées"),
    ("stateFailure_one", "{n} étape en échec"),
    ("stateFailure_other", "{n} étapes en échec"),
    ("stateSuccess", "Tout est validé"),
    ("stateSuccessSkipped", "Aucun échec"),
    ("badgeFailure", "{n} échec(s)"),
    ("send", "Envoyer à l'agent"),
    ("sendAgain", "Renvoyer"),
    ("sending", "Envoi…"),
    (
        "sentTo",
        "Envoyé à {pane}. Il corrige ce qui a échoué : rouvrez cette liste pour la retester.",
    ),
    (
        "needAnswer",
        "Répondez à au moins une étape pour envoyer le résultat.",
    ),
    (
        "pending_one",
        "{n} étape n'a pas de réponse : l'agent en sera informé.",
    ),
    (
        "pending_other",
        "{n} étapes n'ont pas de réponse : l'agent en sera informé.",
    ),
    (
        "noCaller",
        "Aucun agent n'a demandé cette liste. Copiez le résumé pour le donner à l'un d'eux.",
    ),
    ("copy", "Copier le résumé"),
    ("copied", "Copié"),
    ("notifySent", "Résultat envoyé à {pane}"),
    (
        "persistOff",
        "Les réponses ne peuvent pas être enregistrées ici : fermer cet onglet les perd.",
    ),
    (
        "send.denied",
        "Cet onglet ne peut pas écrire aux agents. Autorisez panes.send pour le kit dans Réglages › Kits.",
    ),
    (
        "send.not_found",
        "L'agent qui l'a demandée n'est plus dans cet espace de travail. Copiez plutôt le résumé.",
    ),
    (
        "send.unavailable",
        "La session de cet agent ne peut pas recevoir de message pour l'instant. Réessayez dans un instant.",
    ),
    (
        "send.rate_limited",
        "Trop de messages envoyés. Attendez une minute, puis renvoyez.",
    ),
    (
        "send.error",
        "Le résultat n'a pas pu être envoyé : {detail}",
    ),
    ("err.missing.title", "Aucune liste à afficher"),
    (
        "err.missing.text",
        "Ouvrez cet onglet avec le chemin d'un fichier JSON, par exemple recette/login.json.",
    ),
    ("err.path.title", "Ce chemin ne peut pas servir"),
    (
        "err.path.text",
        "Il doit être relatif au dossier de l'espace de travail, sans .., sans lecteur, sans deux-points et sans dossier caché.",
    ),
    ("err.not_found.title", "Liste introuvable"),
    (
        "err.not_found.text",
        "{path} n'existe pas dans le dossier de l'espace de travail. L'agent écrit le fichier avant d'ouvrir cet onglet.",
    ),
    ("err.denied.title", "Cet onglet ne peut pas lire de fichier"),
    (
        "err.denied.text",
        "Autorisez files.read pour le kit dans Réglages › Kits, puis réessayez.",
    ),
    ("err.json.title", "La liste n'est pas du JSON valide"),
    ("err.json.text", "{detail}"),
    ("err.empty.title", "La liste est vide"),
    (
        "err.empty.text",
        "{path} ne contient rien. L'agent doit y écrire la liste.",
    ),
    ("err.shape.title", "La liste n'a pas la forme attendue"),
    ("err.shape.text", "{detail}"),
    ("err.error.title", "La liste n'a pas pu être lue"),
    ("err.error.text", "{detail}"),
    (
        "shape.notObject",
        "Le fichier doit contenir un objet avec un titre et une liste d'étapes.",
    ),
    (
        "shape.noSteps",
        "steps doit être une liste d'au moins une étape.",
    ),
    (
        "shape.tooMany",
        "{max} étapes au plus ; cette liste en a {n}.",
    ),
    (
        "shape.stepNotObject",
        "L'étape {n} doit être un objet avec un text.",
    ),
    ("shape.stepText", "L'étape {n} n'a pas de text."),
    ("shape.stepExpect", "Étape {n} : expect doit être un texte."),
    (
        "shape.stepLong",
        "Étape {n} : le texte dépasse {max} caractères.",
    ),
    ("shape.titleType", "title doit être un texte."),
];

/// The words in the person's language (`en` or `fr`, or a tag that starts with
/// one; anything else is English).
pub fn strings(locale: &str) -> Strings {
    Strings::new(locale).add("en", EN).add("fr", FR)
}
