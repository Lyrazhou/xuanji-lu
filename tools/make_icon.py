from PIL import Image

source = "/home/ubuntu/xuanji-lu/src-tauri/icons/icon.png"
target = "/home/ubuntu/xuanji-lu/src-tauri/icons/icon.ico"
image = Image.open(source).convert("RGBA")
image.save(target, format="ICO", sizes=[(256, 256), (128, 128), (64, 64), (48, 48), (32, 32), (16, 16)])
