*** Settings ***
Library           OperatingSystem
Library           Process
Library           String
Suite Teardown    Terminate All Processes    kill=True

*** Variables ***
${ROOT}           ${CURDIR}/..
${PY}             python3
${CLIENT}         ${CURDIR}/software_sim_client.py
${CLEAN_CONFIG}   {"mode":"EspNow","tx_power_dbm":0.0,"snr_db":40.0,"filter_bw_mhz":1.0,"threshold":0.5,"noise_level":0.0,"replay_guard":true,"data_bits":"10110010","node_id":1}

*** Test Cases ***
Software Sim Publishes Through ZMQ Sidecar
    ${tmp}=    Create Temp Dir
    ${zmq}=    Set Variable    tcp://127.0.0.1:15556
    Start Process    cargo    run    --quiet    -p    control-plane    --    --health-port    18092    --db-path    ${tmp}/telemetry.db    --zmq-endpoint    ${zmq}    cwd=${ROOT}    stdout=${tmp}/control-plane.log    stderr=STDOUT    alias=control-plane
    Run Process    ${PY}    ${CLIENT}    wait-url    http://127.0.0.1:18092/health    --timeout    45
    Start Process    cargo    run    --quiet    -p    hil-simulator    --    --port    18090    --zmq-endpoint    ${zmq}    cwd=${ROOT}    stdout=${tmp}/hil-simulator.log    stderr=STDOUT    alias=hil-simulator
    Run Process    ${PY}    ${CLIENT}    wait-url    http://127.0.0.1:18090/api/v1/status    --timeout    45
    Run Process    ${PY}    ${CLIENT}    put-json    http://127.0.0.1:18090/api/v1/config    ${CLEAN_CONFIG}
    Sleep    1s
    Run Process    ${PY}    ${CLIENT}    post-json    http://127.0.0.1:18090/api/v1/trigger    {"value":true}
    Run Process    ${PY}    ${CLIENT}    wait-status    http://127.0.0.1:18092/api/v1/live/status    --min-frames    1    --timeout    15
    Run Process    ${PY}    ${CLIENT}    wait-event    http://127.0.0.1:18092/api/v1/live/events    ACTION_TRIGGERED    --timeout    15

Software Sim Publishes Through TLS13 MTLS Ingest
    ${tmp}=    Create Temp Dir
    Run Process    bash    ${CURDIR}/generate_test_certs.sh    ${tmp}/certs
    Start Process    cargo    run    --quiet    -p    control-plane    --    --secure-ingest-only    --health-port    18093    --db-path    ${tmp}/secure.db    --tls-cert    ${tmp}/certs/server.pem    --tls-key    ${tmp}/certs/server.key    --client-ca    ${tmp}/certs/ca.pem    cwd=${ROOT}    stdout=${tmp}/secure-control-plane.log    stderr=STDOUT    alias=secure-control-plane
    Wait Until Keyword Succeeds    45x    1s    Secure Curl Should Succeed    ${tmp}    https://localhost:18093/health
    Curl Without Client Cert Should Fail    ${tmp}    https://localhost:18093/health
    Curl With Wrong Client CA Should Fail    ${tmp}    https://localhost:18093/health
    Plaintext Secure Ingest Should Fail
    TLS12 Secure Curl Should Fail    ${tmp}    https://localhost:18093/health
    Start Process    cargo    run    --quiet    -p    hil-simulator    --    --port    18091    --secure-ingest-url    https://localhost:18093/api/v1/ingest/frame    --tls-cert    ${tmp}/certs/client.pem    --tls-key    ${tmp}/certs/client.key    --server-ca    ${tmp}/certs/ca.pem    cwd=${ROOT}    stdout=${tmp}/secure-hil-simulator.log    stderr=STDOUT    alias=secure-hil-simulator
    Run Process    ${PY}    ${CLIENT}    wait-url    http://127.0.0.1:18091/api/v1/status    --timeout    45
    Run Process    ${PY}    ${CLIENT}    put-json    http://127.0.0.1:18091/api/v1/config    ${CLEAN_CONFIG}
    Run Process    ${PY}    ${CLIENT}    post-json    http://127.0.0.1:18091/api/v1/trigger    {"value":true}
    Wait Until Keyword Succeeds    15x    1s    Secure Events Should Contain    ${tmp}    ACTION_TRIGGERED

*** Keywords ***
Create Temp Dir
    ${tmp}=    Evaluate    tempfile.mkdtemp(prefix="sdr-robot-")    modules=tempfile
    RETURN    ${tmp}

Secure Curl Should Succeed
    [Arguments]    ${tmp}    ${url}
    ${result}=    Run Process    curl    -fsS    --cert    ${tmp}/certs/client.pem    --key    ${tmp}/certs/client.key    --cacert    ${tmp}/certs/ca.pem    ${url}
    Should Be Equal As Integers    ${result.rc}    0

Curl Without Client Cert Should Fail
    [Arguments]    ${tmp}    ${url}
    ${result}=    Run Process    curl    -fsS    --cacert    ${tmp}/certs/ca.pem    ${url}
    Should Not Be Equal As Integers    ${result.rc}    0

Curl With Wrong Client CA Should Fail
    [Arguments]    ${tmp}    ${url}
    ${result}=    Run Process    curl    -fsS    --cert    ${tmp}/certs/wrong-client.pem    --key    ${tmp}/certs/wrong-client.key    --cacert    ${tmp}/certs/ca.pem    ${url}
    Should Not Be Equal As Integers    ${result.rc}    0

Plaintext Secure Ingest Should Fail
    ${result}=    Run Process    curl    -fsS    http://127.0.0.1:18093/api/v1/ingest/frame
    Should Not Be Equal As Integers    ${result.rc}    0

TLS12 Secure Curl Should Fail
    [Arguments]    ${tmp}    ${url}
    ${result}=    Run Process    curl    -fsS    --tls-max    1.2    --cert    ${tmp}/certs/client.pem    --key    ${tmp}/certs/client.key    --cacert    ${tmp}/certs/ca.pem    ${url}
    Should Not Be Equal As Integers    ${result.rc}    0

Secure Events Should Contain
    [Arguments]    ${tmp}    ${text}
    ${result}=    Run Process    curl    -fsS    --cert    ${tmp}/certs/client.pem    --key    ${tmp}/certs/client.key    --cacert    ${tmp}/certs/ca.pem    https://localhost:18093/api/v1/live/events
    Should Be Equal As Integers    ${result.rc}    0
    Should Contain    ${result.stdout}    ${text}
