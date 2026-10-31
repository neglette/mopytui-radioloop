"""Real CLI/curl through a controlled SOCKS5+TLS fixture, despite NO_PROXY/curlrc."""
import json, os, pathlib, socket, ssl, struct, subprocess, sys, tempfile, threading
with tempfile.TemporaryDirectory() as directory:
    base = pathlib.Path(directory)
    cert, key = base/'cert.pem', base/'key.pem'
    subprocess.run(['openssl','req','-x509','-newkey','rsa:2048','-nodes','-days','1','-keyout',str(key),'-out',str(cert),'-subj','/CN=de1.api.radio-browser.info','-addext','subjectAltName=DNS:de1.api.radio-browser.info'],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    (base/'.curlrc').write_text('this-option-does-not-exist = yes\n')
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER); context.load_cert_chain(cert,key)
    server = socket.socket(); server.bind(('127.0.0.1',0)); server.listen(); server.settimeout(15)
    requests=[]; failures=[]
    def exact(connection, count):
        data=b''
        while len(data)<count:
            chunk=connection.recv(count-len(data))
            if not chunk: raise RuntimeError('Unexpected EOF')
            data+=chunk
        return data
    def serve():
        try:
            for _ in range(2):
                connection,_=server.accept(); connection.settimeout(10)
                with connection:
                    version,count=exact(connection,2); exact(connection,count)
                    connection.sendall(b'\x05\x00')
                    version,command,reserved,kind=exact(connection,4)
                    assert kind==3, 'SOCKS must resolve destination remotely'
                    host=exact(connection,exact(connection,1)[0]).decode(); port=struct.unpack('!H',exact(connection,2))[0]
                    assert (host,port)==('de1.api.radio-browser.info',443)
                    connection.sendall(b'\x05\x00\x00\x01\x7f\x00\x00\x01\x01\xbb')
                    with context.wrap_socket(connection,server_side=True) as tls:
                        request=b''
                        while b'\r\n\r\n' not in request: request+=tls.recv(4096)
                        requests.append(request.decode().split('\r\n')[0])
                        tls.sendall(b'HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]')
        except Exception as error: failures.append(repr(error))
    thread=threading.Thread(target=serve,daemon=True); thread.start()
    env=dict(os.environ,MOPYTUI_RADIO_PROXY=f'socks5h://127.0.0.1:{server.getsockname()[1]}',NO_PROXY='*',no_proxy='*',CURL_HOME=str(base),CURL_CA_BUNDLE=str(cert))
    result=subprocess.run([sys.argv[1],'--json','ambient'],env=env,capture_output=True,text=True,timeout=25)
    thread.join(1); server.close()
    assert not failures, failures
    assert result.returncode==0, result.stderr
    assert json.loads(result.stdout)==[], result.stdout
    assert len(requests)==2 and 'name=ambient' in requests[0] and 'tagList=ambient' in requests[1], requests
    print('PASS: real CLI + curl + SOCKS5 remote DNS + TLS; NO_PROXY=* and broken curlrc cannot bypass explicit proxy.')
