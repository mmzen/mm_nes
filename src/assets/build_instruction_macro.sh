#!/usr/bin/bash

pwd
OUTPUT_FILE="../instructions_macro.rs"

addressing_mode() {
  local result=""

  case $1 in
      "implied") result="Implicit" ;;
      "accumulator") result="Accumulator" ;;
      "immediate") result="Immediate" ;;
      "zeropage") result="ZeroPage" ;;
      "zeropage,X") result="ZeroPageIndexedX" ;;
      "zeropage,Y") result="ZeroPageIndexedY" ;;
      "absolute") result="Absolute" ;;
      "absolute,X") result="AbsoluteIndexedX" ;;
      "absolute,Y") result="AbsoluteIndexedY" ;;
      "relative") result="Relative" ;;
      "(indirect)") result="Indirect" ;;
      "(indirect,X)") result="IndirectIndexedX" ;;
      "(indirect),Y") result="IndirectIndexedY" ;;
      *) echo "invalid addressing mode: $1"; exit 1;;
  esac

  echo $result
}

function_name() {
  echo $1 | tr '[:upper:]' '[:lower:]' | sed 's/ /_/g'
}

echo output is $OUTPUT_FILE
echo > $OUTPUT_FILE

cat instructions.txt | sed 1d | while read line ; do
  IFS=";"
  read -r operation description addressing asm opcode bytes cycles <<< "${line}"

  addressing=`addressing_mode $addressing`
  function_name=`function_name "$operation $description"`
  echo "add_instruction(map, 0x${opcode}, ${operation}, ${addressing}, ${bytes}, ${cycles}, ${function_name});" >> $OUTPUT_FILE
done

n=`cat $OUTPUT_FILE | wc -l `
echo "$n macro directives generated"

# add_instruction!(map, 0x00, BRK, Implicit, 2, 7, brk_force_interrupt);