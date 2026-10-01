/* kernel/linker.ld — NeoCore OS Kernel Linker Script */

ENTRY(_start)
OUTPUT_FORMAT(elf64-x86-64)

SECTIONS
{
    /* Kernel dimuat di higher half: 0xFFFFFFFF80000000 */
    . = 0xFFFFFFFF80000000;

    .text ALIGN(4K) :
    {
        KEEP(*(.text.boot))
        *(.text .text.*)
    }

    .rodata ALIGN(4K) :
    {
        *(.rodata .rodata.*)
    }

    .data ALIGN(4K) :
    {
        *(.data .data.*)
    }

    .bss ALIGN(4K) :
    {
        _bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        _bss_end = .;
    }

    /DISCARD/ :
    {
        *(.eh_frame)
        *(.note .note.*)
        *(.comment)
    }
}
