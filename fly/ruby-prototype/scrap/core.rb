module Fly

  module Core

    extend Component

    # getter for top of stack
    word 'top' do
      stack.first
    end

    ### Stack manipulation methods ###

    # remove top of stack (returns nil if empty)
    word 'drop' do
      stack.shift
    end

    # remove top of stack (returns nil if empty)
    #--
    # NOTE could there be location objects (eg. @5) that regular drop
    #       could see is on the stack and use instead?
    #++
    word 'drop_at', [ Integer ] do
      i = pop
      check i
      stack.delete_at( i )
    end

    #
    word 'drop_from', [ Integer ] do
      i = pop
      #check i
      stack.slice!(0,i)
    end

    # remove top of stack (raises error if empty)
    #word 'pop' do
    #  check
    #  stack.shift
    #end

    # empty stack
    word 'clear' do
      stack.clear
    end

    # duplicate the top of the stack
    word 'dup' do
      #return unary do |num|
      #  push( num )
      #  num
      #end
      check
      push stack.first
    end

    # swap top two elements from stack
    word 'swap' do
      #return binary do |l, r|
      #  push( r )
      #  l
      #end
      #r, l = stack.slice!(0,2)
      check 2
      stack[0], stack[1] = stack[1], stack[0]
    end

    word 'over' do
      push stack[1].dup
    end

    # arbitrary stack position swap (up)
    word 'roll' do
      push( drop_at( i ) )
    end

    # arbitrary stack position swap (down)
    word 'rolld' do
      i = pop
      o = pop
      check i
      stack.insert(i, o)
    end

  end

end
