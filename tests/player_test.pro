QT += quick quickcontrols2 multimedia testlib
CONFIG += c++17 console
TARGET = player-test
SOURCES += player_test.cpp ../native/player.cpp
HEADERS += ../native/player.h
RESOURCES += ../resources.qrc

LIBS += $$PWD/../target/release/libnap.a -ldl -lpthread
