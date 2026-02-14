import sys
import json
import importlib.util
import os
import traceback

def main():
    if len(sys.argv) < 2:
        print("Usage: bridge_runtime.py <user_script_path>", file=sys.stderr)
        sys.exit(1)

    script_path = sys.argv[1]
    
    # Load user script
    try:
        spec = importlib.util.spec_from_file_location("user_logic", script_path)
        user_logic = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(user_logic)
    except Exception as e:
        print(f"Error loading user script: {e}", file=sys.stderr)
        traceback.print_exc(file=sys.stderr)
        sys.exit(1)

    # Simple JSON-RPC-like loop over stdin/stdout
    for line in sys.stdin:
        if not line.strip():
            continue
        
        try:
            request = json.loads(line)
            method_name = request.get("method")
            params = request.get("params")
            
            # Built-in methods
            if method_name == "get_manifest":
                manifest = getattr(user_logic, "VERS_MANIFEST", {
                    "id": "python.unnamed",
                    "name": "Unnamed Python Script",
                    "description": "No description provided.",
                    "version": "0.0.0",
                    "capabilities": ["Reasoning"]
                })
                response = {"result": manifest}
            elif hasattr(user_logic, method_name):
                method = getattr(user_logic, method_name)
                # Call the method
                result = method(params)
                response = {"result": result}
            else:
                response = {"error": f"Method '{method_name}' not found in user script"}
        except Exception as e:
            response = {"error": str(e), "traceback": traceback.format_exc()}
        
        # Write response as a single line
        sys.stdout.write(json.dumps(response) + "
")
        sys.stdout.flush()

if __name__ == "__main__":
    main()
