# http_post.vbr — POST a JSON body with request headers.
# 
# This is the shape of an LLM API call: a JSON body, a Content-Type, and an
# `Authorization: Bearer` token. Headers are a HashMap<String, String> (VB's
# Scripting.Dictionary); pass an empty one for no custom headers.

from vbrpy import Ok, Err, _vb, Http

def main():
    key: str = 'sk-demo-key'
    body: str = '{"model": "demo", "prompt": "hello"}'
    headers: dict[str, str] = {}
    headers['Authorization'] = f"Bearer {_vb(key)}"
    headers['Content-Type'] = 'application/json'
    _m0 = Http.post('https://api.example.com/v1/complete', body, headers)
    match _m0:
        case Ok(reply):
            print(f"got {_vb(len(reply))} bytes")
        case Err(message):
            print(f"request failed: {_vb(message)}")


if __name__ == "__main__":
    main()
