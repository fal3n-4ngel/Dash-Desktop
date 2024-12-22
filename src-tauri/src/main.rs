use tauri::Manager;
use std::fs;
use std::io::Write;
use tempfile::NamedTempFile;
use env_logger;
use std::process::Command;
use std::path::PathBuf;
use uuid::Uuid;


fn is_dangerous_command(code: &str) -> bool {
    let dangerous_patterns = [
        "os.system", "subprocess", "shutdown", 
        "rm -rf", "del", "format",
        // Add more patterns
    ];
    
    for pattern in dangerous_patterns {
        if code.contains(pattern) {
            return true;
        }
    }
    false
}

#[tauri::command]
async fn run_python_code(code: String, requirements: Option<String>) -> Result<String, String> {
    // Validate input size
    if code.len() > 10_000 {
        return Err("Code is too large!".to_string());
    }
    if is_dangerous_command(&code) {
        return Err("Potentially dangerous command detected".to_string());
    }

    // Create a unique directory for the virtual environment
    let venv_id = Uuid::new_v4().to_string();
    let venv_path = std::env::temp_dir().join(format!("venv_{}", venv_id));
    
    // Create virtual environment
    let python_executable = std::env::var("PYTHON_EXECUTABLE").unwrap_or("python".to_string());
    let create_venv = Command::new(&python_executable)
        .args(["-m", "venv", venv_path.to_str().unwrap()])
        .output()
        .map_err(|e| format!("Failed to create virtual environment: {}", e))?;

    if !create_venv.status.success() {
        return Err(format!("Failed to create virtual environment: {}", 
            String::from_utf8_lossy(&create_venv.stderr)));
    }

    // Determine the path to the virtual environment's Python executable
    let venv_python = if cfg!(windows) {
        venv_path.join("Scripts").join("python.exe")
    } else {
        venv_path.join("bin").join("python")
    };

    // Install requirements if provided
    if let Some(reqs_str) = requirements {
       

        if !reqs_str.is_empty() {
            // Split the comma-separated string and collect into a Vec
            let reqs: Vec<&str> = reqs_str.split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();

            if !reqs.is_empty() {
                let mut install_cmd = Command::new(venv_python.clone());
                install_cmd.args(["-m", "pip", "install"]);
                install_cmd.args(&reqs);
                
                let install_output = install_cmd
                    .output()
                    .map_err(|e| format!("Failed to install requirements: {}", e))?;

                if !install_output.status.success() {
                    // Clean up virtual environment before returning
                    let _ = fs::remove_dir_all(&venv_path);
                    return Err(format!("Failed to install requirements: {}", 
                        String::from_utf8_lossy(&install_output.stderr)));
                }
            }
        }
    }
    // Create a temporary file for the code
    let mut temp_file = NamedTempFile::new()
        .map_err(|e| format!("Failed to create temp file: {}", e))?;
    
    // Write the code
    temp_file.write_all(code.as_bytes())
        .map_err(|e| format!("Failed to write code: {}", e))?;

    let temp_path = temp_file.into_temp_path();

    // Run the Python code in the virtual environment
    let output = Command::new(venv_python)
        .arg(temp_path.to_str().unwrap())
        .output()
        .map_err(|e| format!("Failed to execute Python: {}", e))?;

    // Clean up the virtual environment
    let _ = fs::remove_dir_all(&venv_path);

    // Handle output
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !stderr.is_empty() {
        Err(format!("Output:\n{}\nErrors:\n{}", stdout, stderr))
    } else {
        Ok(stdout.to_string())
    }
}


fn main() {
    env_logger::init();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![run_python_code])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}