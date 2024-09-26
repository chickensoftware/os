BOOTLOADER_DIR = chicken-loader
KERNEL_DIR = chicken-kernel
UTIL_DIR = chicken-util

TARGET_DIR_BOOTLOADER_DEBUG = target/x86_64-unknown-uefi/debug
TARGET_DIR_BOOTLOADER_RELEASE = target/x86_64-unknown-uefi/release

TARGET_DIR_KERNEL_DEBUG = target/x86_64-unknown-none-chicken/debug
TARGET_DIR_KERNEL_RELEASE = target/x86_64-unknown-none-chicken/release

EFI_FILE = chicken-loader.efi
KERNEL_FILE = chicken-kernel.elf
FONT_FILE = light16.psf  # currently light16.psf or ext-light32.psf
FONT_DIR = $(UTIL_DIR)/fonts

BUILD_DIR = build
ESP_DIR = $(BUILD_DIR)/esp
BOOT_DIR = $(ESP_DIR)/efi/boot

QEMU_LOG = qemu.log

OVMF_DIR = /usr/share/OVMF/x64
OVMF_CODE = $(OVMF_DIR)/OVMF_CODE.fd
OVMF_VARS = $(OVMF_DIR)/OVMF_VARS.fd

USB_DEVICE = /dev/zero

ifdef release
    CARGO_CMD = cargo build --release --target-dir=../target
    TARGET_DIR_BOOTLOADER = $(TARGET_DIR_BOOTLOADER_RELEASE)
    TARGET_DIR_KERNEL = $(TARGET_DIR_KERNEL_RELEASE)
else
    CARGO_CMD = cargo build --target-dir=../target
    TARGET_DIR_BOOTLOADER = $(TARGET_DIR_BOOTLOADER_DEBUG)
    TARGET_DIR_KERNEL = $(TARGET_DIR_KERNEL_DEBUG)
endif

.PHONY: all
all: bootloader kernel
	@echo "Build complete."

.PHONY: bootloader
bootloader:
	@echo "Building bootloader..."
	@cd $(BOOTLOADER_DIR) && $(CARGO_CMD)

.PHONY: kernel
kernel:
	@echo "Building kernel..."
	@cd $(KERNEL_DIR) && $(CARGO_CMD)

.PHONY: clippy
clippy:
	@echo "Running clippy..."
	@cd $(BOOTLOADER_DIR) && cargo clippy --target-dir=../target
	@cd $(KERNEL_DIR) && cargo clippy --target-dir=../target
	@cd $(UTIL_DIR) && cargo clippy --target-dir=../target

.PHONY: clean
clean:
	@echo "Cleaning target directory..."
	@cargo clean
	@echo "Cleaning build directory..."
	@rm -rf $(BUILD_DIR)
	@echo "Clean complete."

.PHONY: run
run: all
	@echo "Creating build directory..."
	@mkdir -p $(BOOT_DIR)
	@echo "Copying UEFI file to boot directory..."
	@cp $(TARGET_DIR_BOOTLOADER)/$(EFI_FILE) $(BOOT_DIR)/bootx64.efi
	@echo "Copying kernel file to boot directory..."
	@cp $(TARGET_DIR_KERNEL)/$(KERNEL_FILE) $(ESP_DIR)/kernel.elf
	@echo "Copying font file to boot directory..."
	@cp $(FONT_DIR)/$(FONT_FILE) $(ESP_DIR)/font.psf
	@echo "Running QEMU..."
	@qemu-system-x86_64 \
		-drive if=pflash,format=raw,readonly=on,file=$(OVMF_CODE) \
		-drive if=pflash,format=raw,readonly=on,file=$(OVMF_VARS) \
		-drive format=raw,file=fat:rw:$(ESP_DIR)  \
		-D $(QEMU_LOG) -no-reboot -serial stdio -m 256M

.PHONY: usb
usb: all
	@echo "Formatting USB drive..."
	@sudo parted $(USB_DEVICE) -- mklabel gpt
	@sudo parted $(USB_DEVICE) -- mkpart ESP fat32 1MiB 100%
	@sudo parted $(USB_DEVICE) -- set 1 esp on
	@sudo mkfs.fat -F32 $(USB_DEVICE)1
	@echo "Mounting USB drive..."
	@sudo mount $(USB_DEVICE)1 /mnt
	@echo "Creating EFI boot directory on USB drive..."
	@sudo mkdir -p /mnt/efi/boot
	@echo "Copying UEFI file to USB drive..."
	@sudo cp $(TARGET_DIR_BOOTLOADER)/$(EFI_FILE) /mnt/efi/boot/bootx64.efi
	@echo "Copying kernel file to USB drive..."
	@sudo cp $(TARGET_DIR_KERNEL)/$(KERNEL_FILE) /mnt/kernel.elf
	@echo "Copying font file to boot directory..."
	@sudo cp $(FONT_DIR)/$(FONT_FILE) /mnt/font.psf
	@echo "Unmounting USB drive..."
	@sudo umount /mnt
	@echo "USB drive is ready to boot."
