# settings: status_mode=1
# settings: gestures=0
# rust-only
# env: GRIMOIRE_PACKAGE_STORE_OFFLINE=1
# copy: tests/fixtures/apppkg .
# copy: tests/fixtures/package-store .
# Apps: an installed type: app package is listed in the Package Store's Installed tab, the All apps
# page's Manage apps... manages it exactly like widgets, and the installed app is a launcher in the
# home app row and All Apps page. Opening it gives it the whole screen and the status bar only when it
# asks; a tap closes it through its own close() API, and the two-finger bottom-to-middle system swipe
# returns Home from a fresh launch even with Gestures off.
wait 300
expect library.apps is com.example.fixture
# The app's bundled icon is decoded by the sandboxed worker (issue #236): its picture comes a moment after the
# first draw, and every launcher below shows it (tests/scenarios/z_apps_icon.qs holds how it arrives).
waitfor library.apps.icons is 1
# The Store's Installed tab (the saved catalog, offline) lists the app with the installed packages.
tap library app.store
wait 400
expect library.sheet is store
tap library store.filter.4
wait 300
expect library.store.filter is 4
expect library.store.visible is com.example.fixture
shot 00-store-installed
tap library store.close
wait 300
expect library.sheet is none
expect screen is library
tap library apps
wait 200
shot 04-all-apps
# The All apps page's Manage apps... opens the app package manager over the page, as the widget
# popup's Manage widgets... opens the widget one: its list, a package's page and its settings, which
# are saved as they are tapped. Back and the X come back to the All apps page.
tap library apps.manage
wait 400
expect editor.packages.open is 1
expect editor.packages is 1 com.example.fixture 1.0.0
expect editor.package.com.example.fixture is 1.0.0
expect editor.package.com.example.fixture.settings.dark is true
shot 01-apps-manager
tap library packages.row.com.example.fixture
wait 300
shot 02-app-package-detail
tap library packages.settings
wait 300
shot 03-app-package-settings
tap library packages.setting.dark
wait 200
expect editor.package.com.example.fixture.settings.dark is false
tap library packages.back
wait 200
tap library packages.back
wait 300
tap library packages.back
wait 300
expect editor.packages.open is 0
expect screen is library
tap library apps.manage
wait 400
expect editor.packages.open is 1
tap library packages.close
wait 300
expect editor.packages.open is 0
# (the page's own X is there again: it is the All apps page, not Home)
tap library apps.close
wait 300
expect library.sheet is none
tap library apps
wait 200
tap library app.com.example.fixture
wait 400
expect library.app.open is 1
expect library.app.id is com.example.fixture
expect library.app.status is fixture app
expect library.app.status-bar is 0
shot 05-app
# Quick settings is a system layer even over an app that hides the status bar: a one-finger pull
# from the top opens the same panel.
touch down 930 30
touch move 930 120
wait 150
expect quick is 1
touch up 930 120
wait 100
shot 05b-app-quick-settings
tap panel quick.close
wait 200
expect quick is 0
# The app's own close() API: its app_pointer calls it on a tap's lift.
tap 930 1300
wait 300
expect library.app.open is 0
shot 06-app-closed
# Launch again and use the system gesture, which no app can disable.
tap library app.com.example.fixture
wait 400
expect library.app.open is 1
swipe 2 930 2200 0 -1000
wait 300
expect library.app.open is 0
expect screen is library
shot 07-app-system-home
# The app is first on the home row. Opening Settings must move Settings ahead of it, not push the
# app behind every built-in.
expect library.recent-apps is pkg:com.example.fixture,store,files,share,settings
tap library app.settings
wait 300
expect library.sheet is settings
expect library.recent-apps is settings,pkg:com.example.fixture,store,files,share
shot 08-settings-after-app
# Settings' main page closes with its X ("done"), unlike its submenus' settings.close.
tap library done
wait 300
expect library.sheet is none
shot 09-home-order
quit
