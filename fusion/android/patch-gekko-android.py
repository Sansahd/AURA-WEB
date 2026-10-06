from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SERVO = ROOT / "vendor" / "servo"


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text(encoding="utf-8")
    if new in text:
        return
    if old not in text:
        raise SystemExit(f"Android Gekko patch anchor missing: {path} :: {old[:90]!r}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


gradle = SERVO / "support/android/apk/servoapp/build.gradle.kts"
replace_once(gradle, 'applicationId = "org.servo.servoshell"', 'applicationId = "com.quantic.gekko"')
replace_once(gradle, 'versionName = "0.7.0"', 'versionName = "0.8.5"')

strings = SERVO / "support/android/apk/servoapp/src/main/res/values/strings.xml"
translations = {
    '<string name="app_name">Servo</string>': '<string name="app_name">Quantic Gekko</string>',
    '<string name="back">Back</string>': '<string name="back">Retour</string>',
    '<string name="media_channel_name">ServoMedia</string>': '<string name="media_channel_name">Gekko Media</string>',
    '<string name="media_channel_description">Notication channel for multimedia activity</string>': '<string name="media_channel_description">Contrôles multimédias de Gekko</string>',
    '<string name="history_back">Back</string>': '<string name="history_back">Retour</string>',
    '<string name="history_forward">Forward</string>': '<string name="history_forward">Suivant</string>',
    '<string name="options">Options</string>': '<string name="options">Réglages</string>',
    '<string name="url_or_search">URL or Search</string>': '<string name="url_or_search">Rechercher ou saisir une adresse</string>',
    '<string name="refresh">Refresh</string>': '<string name="refresh">Actualiser</string>',
    '<string name="cancel">Cancel</string>': '<string name="cancel">Arrêter</string>',
    '<string name="history_title">History</string>': '<string name="history_title">Historique</string>',
    '<string name="clear_history">Clear History</string>': '<string name="clear_history">Effacer l’historique</string>',
}
text = strings.read_text(encoding="utf-8")
for old, new in translations.items():
    text = text.replace(old, new)
strings.write_text(text, encoding="utf-8")

manifest = SERVO / "support/android/apk/servoapp/src/main/AndroidManifest.xml"
replace_once(manifest, 'android:icon="@mipmap/servo"', 'android:icon="@drawable/gekko_mark"')

drawable = SERVO / "support/android/apk/servoapp/src/main/res/drawable/gekko_mark.xml"
drawable.parent.mkdir(parents=True, exist_ok=True)
drawable.write_text(
    """<?xml version="1.0" encoding="utf-8"?>
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="108dp"
    android:height="108dp"
    android:viewportWidth="256"
    android:viewportHeight="256">
    <path
        android:fillColor="#161C18"
        android:strokeColor="#F6B540"
        android:strokeWidth="9"
        android:pathData="M118,60 A68,68 0,1 0,118 196 A68,68 0,1 0,118 60" />
    <path
        android:fillColor="@android:color/transparent"
        android:strokeColor="#B99952"
        android:strokeWidth="4"
        android:pathData="M55,128 L181,128 M118,62 L118,194" />
    <path
        android:fillColor="@android:color/transparent"
        android:strokeColor="#708471"
        android:strokeWidth="3"
        android:pathData="M118,60 C96,60 79,90 79,128 C79,166 96,196 118,196 C140,196 157,166 157,128 C157,90 140,60 118,60" />
    <path
        android:fillColor="@android:color/transparent"
        android:strokeColor="#DFBE54"
        android:strokeWidth="17"
        android:strokeLineCap="round"
        android:strokeLineJoin="round"
        android:pathData="M159,76 C183,85 198,104 200,127 C202,151 192,174 171,193 C163,200 153,206 142,210" />
    <path
        android:fillColor="#F6D363"
        android:pathData="M158,62 A13,13 0,1 0,158 88 A13,13 0,1 0,158 62" />
    <path
        android:fillColor="@android:color/transparent"
        android:strokeColor="#DFBE54"
        android:strokeWidth="7"
        android:strokeLineCap="round"
        android:pathData="M176,98 L153,80 M197,126 L221,115 M189,161 L168,180 M167,191 L184,209" />
</vector>
""",
    encoding="utf-8",
)

main_activity = SERVO / "support/android/apk/servoapp/src/main/java/org/servo/servoshell/MainActivity.kt"
text = main_activity.read_text(encoding="utf-8")
text = text.replace(
    "import androidx.compose.foundation.layout.Row\n",
    "import androidx.compose.foundation.layout.Column\nimport androidx.compose.foundation.layout.fillMaxWidth\n",
)
text = text.replace(
    'if (Intent.ACTION_VIEW == intent.action) intent.data.toString() else null',
    'if (Intent.ACTION_VIEW == intent.action) intent.data.toString() else "https://mediumorchid-badger-314305.hostingersite.com"',
)

scaffold = text.index("            Scaffold(\n")
props_start = text.index("                topBar = {", scaffold)
props_end = text.index("            ) { innerPadding ->", props_start)
bottom = '''                bottomBar = {
                    Column {
                        Omnibox(
                            urlTextFieldState,
                            onSearch = { search ->
                                servoView.loadUri(search)
                                servoView.requestFocus()
                            },
                            modifier =
                                Modifier.fillMaxWidth()
                                    .padding(horizontal = 10.dp, vertical = 6.dp),
                        )
                        NavigationBar {
                            NavigationBarItem(
                                selected = false,
                                enabled = navigator.canGoBackState.value,
                                onClick = { onHistoryBackMenuItemClicked(navigator) },
                                icon = { Icon(painterResource(R.drawable.arrow_back), null) },
                                label = { Text(stringResource(R.string.history_back)) },
                            )
                            NavigationBarItem(
                                selected = false,
                                enabled = navigator.canGoForwardState.value,
                                onClick = { onHistoryForwardMenuItemClicked(navigator) },
                                icon = { Icon(painterResource(R.drawable.arrow_forward), null) },
                                label = { Text(stringResource(R.string.history_forward)) },
                            )
                            NavigationBarItem(
                                selected = false,
                                onClick = {
                                    if (isRefreshingState.value) onCancelMenuItemClicked()
                                    else onRefreshMenuItemClicked(navigator)
                                },
                                icon = {
                                    Icon(
                                        painterResource(
                                            if (isRefreshingState.value) R.drawable.cancel
                                            else R.drawable.refresh
                                        ),
                                        null,
                                    )
                                },
                                label = {
                                    Text(
                                        stringResource(
                                            if (isRefreshingState.value) R.string.cancel
                                            else R.string.refresh
                                        )
                                    )
                                },
                            )
                            NavigationBarItem(
                                selected = false,
                                onClick = ::onSettingsMenuItemClicked,
                                icon = { Icon(painterResource(R.drawable.settings), null) },
                                label = { Text(stringResource(R.string.options)) },
                            )
                            NavigationBarItem(
                                selected = false,
                                onClick = ::onHistoryMenuItemClicked,
                                icon = { Icon(painterResource(R.drawable.history), null) },
                                label = { Text(stringResource(R.string.history_title)) },
                            )
                        }
                    }
                },
'''
text = text[:props_start] + bottom + text[props_end:]
main_activity.write_text(text, encoding="utf-8")

android_rust = SERVO / "ports/servoshell/egl/android/mod.rs"
replace_once(
    android_rust,
    '        preferences.set_value("viewport_meta_enabled", servo::PrefValue::Bool(true));',
    '''        preferences.set_value("viewport_meta_enabled", servo::PrefValue::Bool(true));
        // Gekko Privacy Shield defaults on Android. The mobile shell has no
        // permissive fallback: sensitive APIs stay off until a dedicated
        // per-site permission surface is implemented.
        preferences.set_value("dom_webrtc_enabled", servo::PrefValue::Bool(false));
        preferences.set_value("dom_webrtc_transceiver_enabled", servo::PrefValue::Bool(false));
        preferences.set_value("dom_bluetooth_enabled", servo::PrefValue::Bool(false));
        preferences.set_value("dom_geolocation_enabled", servo::PrefValue::Bool(false));
        preferences.set_value("dom_notification_enabled", servo::PrefValue::Bool(false));''',
)

print("Gekko Android patch applied")
