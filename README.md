# 🌟 **File2Link** – Your Reliable File Sharing and Storage Assistant! 🌟

**File2Link** – is a simple and convenient bot that allows you to quickly and securely upload files to a server and
receive a download link. Use it for file sharing or for quick file transfers to the server.

## 📋 **Features:**

- **File Upload:** Send your files to the bot, and it will save them on the server.
- **Get a Download Link:** After the upload, you will receive a unique link to download the file.
- **Easy to Use:** Simple interface for uploading and receiving files.

## 🚀 **How It Works:**

1. **Send a File to the Bot:** You can upload any file you wish to save or share.
2. **Bot Saves the File on the Server:** Your file will be stored on the server where the bot is running.
3. **Receive the Download Link:** The bot will provide you with a link to download the uploaded file.

## 📂 **Use Cases:**

- **File Sharing:** Quickly share documents, images, or videos with friends and colleagues.
- **Data Transfer to Server:** Upload files for backup or further processing.

## 🧩 **Installation and Setup**

### 🐳 Run Using Docker Engine

**Files uploaded via the bot are stored in the `/files` directory within the Docker container.** The `volumes` section
in the `docker-compose.yml` file maps this directory to `/path/to/store/files` on your host machine. This means you can
access the files through `/path/to/store/files` on your host machine.

You can easily run **File2Link** using Docker. Follow these steps to get started:

1. **Clone the Repository**

   Clone the repository from GitHub to your local machine:

   ```bash
   git clone https://github.com/farneser/file2link.git
   cd file2link
   ```

2. **Set Up Environment Variables**

   Create a `.env` file in the root directory based on the `.env.example` file and add the following variables:

   ```text
   # application
   BOT_TOKEN=123456789:abcdefghijklmnop
   SERVER_PORT=8081
   APP_DOMAIN=http://localhost:8081
   TELEGRAM_API_URL=http://nginx:80
   RUST_LOG=info
   
   # telegram bot api server
   TELEGRAM_API_ID=1234567
   TELEGRAM_API_HASH=abcdefghijklmnopqrstuvwxyz0123456789
   TELEGRAM_LOCAL=true
   ```
   Here's a breakdown of each environment variable:

    - **`BOT_TOKEN`**: Your Telegram bot token, which you can obtain
      from [BotFather](https://core.telegram.org/bots#botfather).

      Example:
      ```text
      BOT_TOKEN=123456789:abcdefghijklmnop
      ```

    - **`SERVER_PORT`**: The port on which the application will run.

      Default:
      ```text
      SERVER_PORT=8080
      ```

    - **`APP_DOMAIN`**: The domain or IP address where your application is accessible.

      Default:
      ```text
      APP_DOMAIN=http://localhost:8080
      ```

      Example:
      ```text
      APP_DOMAIN=https://domain.com
      ```

    - **`TELEGRAM_API_URL`**: The URL of the Telegram API server. If you are running the API server in Docker, it’s
      usually the name of the Docker service.

      Default:
      ```text
      TELEGRAM_API_URL=https://api.telegram.org
      ```

    - **`RUST_LOG`**: Log level for the Rust application.

      Default:
      ```text
      RUST_LOG=info
      ```

    - **`TELEGRAM_API_ID`**: Your Telegram API ID, which you can obtain
      from [my.telegram.org](https://my.telegram.org/).

      Example:
      ```text
      TELEGRAM_API_ID=1234567
      ```

    - **`TELEGRAM_API_HASH`**: Your Telegram API hash, which you can obtain
      from [my.telegram.org](https://my.telegram.org/).

      Example:
      ```text
      TELEGRAM_API_HASH=abcdefghijklmnopqrstuvwxyz0123456789
      ```

    - **`TELEGRAM_LOCAL`**: Set to `true` to indicate that the bot is running on a local environment.

      Default:
      ```text
      TELEGRAM_LOCAL=true
      ```

3. **Build the Docker Image**

   Build the Docker image for the bot:

   ```bash
   docker compose build 
   ```

4. **Run the Docker Container**

   Run the Docker container with the environment variables:

   ```bash
   docker compose run
   ```

   Make sure to replace `/path/to/store/files` with the path you used in the `.env` file for file storage.

---

**File2Link** – The perfect solution for easy and efficient file management!

🌟 Try it out now and see how easy file handling can be! 🌟
