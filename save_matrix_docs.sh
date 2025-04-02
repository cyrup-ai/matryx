#!/bin/bash

# Create main index file
echo "# Matrix SDK Documentation" > ./docs/matrix-sdk/index.md
echo "" >> ./docs/matrix-sdk/index.md
echo "This documentation was scraped from the official Matrix Rust SDK docs." >> ./docs/matrix-sdk/index.md
echo "" >> ./docs/matrix-sdk/index.md
echo "## Main Components" >> ./docs/matrix-sdk/index.md
echo "" >> ./docs/matrix-sdk/index.md
echo "- [Client](client.md) - The main client interface" >> ./docs/matrix-sdk/index.md
echo "- [Room](room/index.md) - High-level room API" >> ./docs/matrix-sdk/index.md
echo "- [Authentication](authentication/index.md) - Authentication-related functionality" >> ./docs/matrix-sdk/index.md
echo "- [Config](config/index.md) - Client configuration" >> ./docs/matrix-sdk/index.md
echo "- [Media](media/index.md) - Media API for handling files and attachments" >> ./docs/matrix-sdk/index.md
echo "- [Encryption](encryption/index.md) - End-to-end encryption features" >> ./docs/matrix-sdk/index.md
echo "" >> ./docs/matrix-sdk/index.md
echo "## Full Module List" >> ./docs/matrix-sdk/index.md
echo "" >> ./docs/matrix-sdk/index.md
echo "See the [modules](modules.md) page for a complete list of available modules." >> ./docs/matrix-sdk/index.md

# Create module subindex files
echo "# Room API" > ./docs/matrix-sdk/room/index.md
echo "" >> ./docs/matrix-sdk/room/index.md
echo "The Room API provides high-level functionality for interacting with Matrix rooms." >> ./docs/matrix-sdk/room/index.md

echo "# Authentication API" > ./docs/matrix-sdk/authentication/index.md
echo "" >> ./docs/matrix-sdk/authentication/index.md 
echo "The Authentication API provides methods for logging in, registering, and managing sessions." >> ./docs/matrix-sdk/authentication/index.md

echo "# Configuration API" > ./docs/matrix-sdk/config/index.md
echo "" >> ./docs/matrix-sdk/config/index.md
echo "The Configuration API provides methods for configuring the Matrix client." >> ./docs/matrix-sdk/config/index.md

echo "# Media API" > ./docs/matrix-sdk/media/index.md
echo "" >> ./docs/matrix-sdk/media/index.md
echo "The Media API provides methods for uploading, downloading, and managing media in Matrix." >> ./docs/matrix-sdk/media/index.md

echo "# Encryption API" > ./docs/matrix-sdk/encryption/index.md
echo "" >> ./docs/matrix-sdk/encryption/index.md
echo "The Encryption API provides methods for end-to-end encryption in Matrix." >> ./docs/matrix-sdk/encryption/index.md

echo "Documentation saved successfully!"