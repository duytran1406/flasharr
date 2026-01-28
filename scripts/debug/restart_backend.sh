#!/bin/bash
pkill flasharr
sleep 2
./target/release/flasharr &
echo "Restarted"
