# settings: status_mode=1
# rust-only
# env: GRIMOIRE_PACKAGE_STORE_OFFLINE=1
# copy: tests/fixtures/widgetpkg .
# copy: tests/fixtures/package-store .
# The widget package manager: an installed package appears in the Package Store's Installed tab and in
# the widget popup, the popup's Manage widgets... opens the package manager, a frame opens from the
# popup, Remove asks first, and Install from repository... opens the repository keyboard.
# (The app repaints once more about 3.5 s after it starts, mainloop.rs: the scenario starts after that, so the check
# below that a tap on empty paper refreshes nothing never meets it on a fast machine, issue #226.)
wait 3600
# From the Library: the Package Store's Installed tab lists the package (the saved catalog, offline),
# then the X closes the Store back to the Library.
tap library app.store
wait 400
expect library.sheet is store
tap library store.filter.4
wait 300
expect library.store.filter is 4
expect library.store.visible is com.example.timer
tap library store.close
wait 300
expect library.sheet is none
expect screen is library
# Open a notebook for the widget popup and the widget frames.
tap library add
tap library add.new-notebook
wait 100
tap 1564 2254
wait 300
expect screen is editor
# The popup's Manage widgets... opens the package manager, over the notebook as Settings is.
tap panel widgets
wait 300
shot 00-widgets-popup
tap panel widgets.manage
wait 400
expect editor.packages.open is 1
expect editor.packages is 1 com.example.timer 1.0.0
expect editor.package.com.example.timer is 1.0.0
shot 01-packages
# A tap that finds no control (the empty part of the page) draws and refreshes nothing:
# the page is not redrawn for a contact that changed nothing.
expect display.presents not -1
tap 900 1500
wait 120
expect display.presents is
# The X closes the page and Settings under it, back to the notebook. The installed package is in
# the popup too, and opens as a frame.
tap panel packages.close
wait 300
expect editor.packages.open is 0
expect screen is editor
tap panel widgets
wait 300
tap panel widgets.com.example.timer
wait 400
expect editor.widgets is 1 com.example.timer
shot 02-package-widget
tap panel widget.com.example.timer.close
wait 300
# Its detail page, then Remove with its question, from the popup's Manage widgets... again.
tap panel widgets
wait 200
tap panel widgets.manage
wait 400
tap panel packages.row.com.example.timer
wait 300
shot 03-package-detail
# The package's own settings: manifest-defined rows, saved as they are tapped, and reset together.
tap panel packages.settings
wait 300
expect editor.package.com.example.timer.settings.show_seconds is true
expect editor.package.com.example.timer.settings.layout is compact
shot 03a-package-settings
tap panel packages.setting.show_seconds
wait 200
expect editor.package.com.example.timer.settings.show_seconds is false
tap panel packages.setting.layout
wait 200
expect editor.package.com.example.timer.settings.layout is large
shot 03b-package-settings-changed
tap panel packages.settings-reset
wait 200
expect editor.package.com.example.timer.settings.show_seconds is true
expect editor.package.com.example.timer.settings.layout is compact
tap panel packages.back
wait 300
tap panel packages.remove
wait 300
shot 04-remove-question
tap panel packages.remove-yes
wait 400
expect editor.packages is 0
expect editor.packages.open is 1
shot 05-package-removed
tap panel packages.install
wait 400
shot 06-install-keyboard
# The repository is typed on the one global keyboard: a key refreshes its own line and key on the
# fast waveform, never the page under it (the page is not drawn again until Done or Cancel).
expect keyboard.draws not -1
expect display.presents not -1
tap 160 1787
wait 100
expect keyboard.draws is 2
expect display.presents has fast
expect display.presents not has full
expect display.presents not has ui
shot 07-install-keyboard-typed
quit
