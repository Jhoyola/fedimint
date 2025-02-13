#!/usr/bin/env bash

alias lightning-cli="\$FM_LIGHTNING_CLI"
alias lncli="\$FM_LNCLI"
alias bitcoin-cli="\$FM_BTC_CLIENT"
alias fedimint-cli="\$FM_MINT_CLIENT"
alias gateway-cln="\$FM_GWCLI_CLN"
alias gateway-lnd="\$FM_GWCLI_LND"
alias fedimint-dbtool-server-0="env FM_DBTOOL_CONFIG_DIR=\$FM_DATA_DIR/server-0 FM_PASSWORD=pass \$FM_DB_TOOL --database \$FM_DATA_DIR/server-0/database"
alias fedimint-dbtool-server-1="env FM_DBTOOL_CONFIG_DIR=\$FM_DATA_DIR/server-1 FM_PASSWORD=pass \$FM_DB_TOOL --database \$FM_DATA_DIR/server-1/database"
alias fedimint-dbtool-server-2="env FM_DBTOOL_CONFIG_DIR=\$FM_DATA_DIR/server-2 FM_PASSWORD=pass \$FM_DB_TOOL --database \$FM_DATA_DIR/server-2/database"
alias fedimint-dbtool-server-3="env FM_DBTOOL_CONFIG_DIR=\$FM_DATA_DIR/server-3 FM_PASSWORD=pass \$FM_DB_TOOL --database \$FM_DATA_DIR/server-3/database"
