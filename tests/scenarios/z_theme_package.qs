# settings: status_mode=1
# rust-only
# env: GRIMOIRE_PACKAGE_STORE_OFFLINE=1
# copy: tests/fixtures/package-store .
# copy: tests/fixtures/themepkg .
# Theme packages: a type: theme package left in packages/import/ is installed as the Store opens, listed in its
# Installed tab, and is a theme of Settings > Display > Style after the built-in ones. Picked, it draws Grimoire
# with its theme.ini's corners and caps; uninstalled while in use, Gothic comes back.
wait 300
tap library app.store
wait 400
expect library.sheet is store
expect library.store.dev.note is Imported from packages/import: Imported Paper Cut
tap library store.filter.4
wait 300
expect library.store.visible is com.example.paper-cut
tap library store.close
wait 300
tap library settings
wait 200
tap library settings.cat.1
wait 100
tap library theme
wait 200
expect library.sheet is style
shot 01-style-list
tap library theme.com.example.paper-cut
wait 300
expect settings.theme is com.example.paper-cut
expect library.sheet is settings
shot 02-settings-paper-cut
tap library done
wait 300
shot 03-home-paper-cut
# Uninstalled from the Store while it is the theme in use: Gothic again, and settings.ini says so.
tap library app.store
wait 400
tap library store.filter.4
wait 300
tap library store.package.com.example.paper-cut
wait 300
tap library store.uninstall
wait 200
tap library store.uninstall-yes
wait 300
expect library.store.status is Uninstalled Paper Cut
expect settings.theme is gothic
