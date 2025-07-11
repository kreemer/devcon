#!/bin/bash

# MIT License
#
# Copyright (c) 2025 DevCon Contributors
#
# Permission is hereby granted, free of charge, to any person obtaining a copy
# of this software and associated documentation files (the "Software"), to deal
# in the Software without restriction, including without limitation the rights
# to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
# copies of the Software, and to permit persons to whom the Software is
# furnished to do so, subject to the following conditions:
#
# The above copyright notice and this permission notice shall be included in all
# copies or substantial portions of the Software.
#
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
# IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
# FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
# AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
# LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
# OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
# SOFTWARE.

# DevCon Browser Helper Script
# This script allows opening URLs in the host's default browser from within a devcontainer
# Usage: ./devcon-browser.sh <url>

SOCKET_PATH="/tmp/devcon-browser.sock"

if [ $# -eq 0 ]; then
    echo "Usage: $0 <url>"
    echo "Example: $0 https://github.com"
    exit 1
fi

URL="$1"

if [ ! -S "$SOCKET_PATH" ]; then
    echo "Error: DevCon browser socket not found at $SOCKET_PATH"
    echo "Make sure the devcon socket server is running on the host:"
    echo "  devcon socket --daemon"
    exit 1
fi

# Send the URL to the socket and read the response
echo "$URL" | nc -U "$SOCKET_PATH"
