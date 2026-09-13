.PHONY: all test clean install uninstall
PREFIX ?= $(HOME)/.local
all:
	mkdir -p build
	cd build && /usr/lib/qt6/bin/qmake ../nap.pro && $(MAKE)
test: all
	$(CXX) -std=c++17 -fPIC tests/util_test.cpp $$(pkg-config --cflags --libs Qt6Core) -o build/util-test
	./build/util-test
	mkdir -p build/tests
	cd build/tests && /usr/lib/qt6/bin/qmake ../../tests/player_test.pro && $(MAKE)
	QT_QPA_PLATFORM=offscreen QT_QPA_PLATFORMTHEME=generic QT_QUICK_BACKEND=software QT_QUICK_CONTROLS_STYLE=Basic ./build/tests/player-test
	python tests/smoke.py
install: all
	install -Dm755 build/nap $(DESTDIR)$(PREFIX)/bin/nap
	install -Dm644 nap.desktop $(DESTDIR)$(PREFIX)/share/applications/nap.desktop
uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/nap $(DESTDIR)$(PREFIX)/share/applications/nap.desktop
clean:
	rm -rf build
