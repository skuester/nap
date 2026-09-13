QT += quick quickcontrols2 multimedia testlib
CONFIG += c++17 console
TARGET = player-test
SOURCES += player_test.cpp ../src/player.cpp
HEADERS += ../src/player.h
RESOURCES += ../resources.qrc
