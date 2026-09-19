QT += quick quickcontrols2 multimedia testlib
CONFIG += c++17 console
TARGET = player-test
SOURCES += player_test.cpp ../native/player.cpp
HEADERS += ../native/player.h
RESOURCES += ../resources.qrc

# Relink when the Rust core changes, not only when the test's own sources do.
PRE_TARGETDEPS += $$PWD/../target/release/libnap.a
LIBS += $$PWD/../target/release/libnap.a -ldl -lpthread
